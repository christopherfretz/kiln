//! The byte level: [`Writer`] and [`Reader`].
//!
//! Every encoding decision below the [`Snap`] impls lives here, so that a
//! later package can change it in one place (spec/snap.md "Wire format"):
//! - fixed-width integers are little-endian, at their natural width;
//! - lengths and counts ([`Writer::length`]) are unsigned LEB128;
//! - nothing is aligned and there is no framing: a snapshot is the
//!   concatenation of its values' encodings in `put` order.

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;

use crate::Snap;
use crate::error::{ErrorKind, SnapError};

/// Appends encoded values to a byte buffer.
///
/// Holds the sharing table of [`Rc`](std::rc::Rc)/[`Arc`](std::sync::Arc)
/// allocations already written, keyed on their address: every object
/// encoded through one writer must stay alive until the writer is done,
/// or a later allocation at a reused address would be taken for it.
pub struct Writer {
    buf: Vec<u8>,
    canonical: bool,
    /// `(address, type) -> shared index`, in order of first write.
    shared: HashMap<(usize, TypeId), u64>,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Writer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Writer")
            .field("len", &self.buf.len())
            .field("canonical", &self.canonical)
            .field("shared", &self.shared.len())
            .finish()
    }
}

impl Writer {
    /// A writer in the normal mode: hash maps and sets are written in
    /// iteration order.
    pub fn new() -> Self {
        Writer { buf: Vec::new(), canonical: false, shared: HashMap::new() }
    }

    /// A writer in canonical mode: hash maps and sets are written sorted
    /// by their entries' encoded bytes, with their length in place of
    /// their capacity, so that two equal states encode to equal bytes
    /// whatever their tables' history. Slower; for equality tests.
    pub fn canonical() -> Self {
        Writer { canonical: true, ..Self::new() }
    }

    /// True for a [`Writer::canonical`] writer.
    pub fn is_canonical(&self) -> bool {
        self.canonical
    }

    /// Bytes written so far.
    pub fn position(&self) -> usize {
        self.buf.len()
    }

    /// The bytes written so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// The encoded bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Reserves room for `n` more bytes.
    pub fn reserve(&mut self, n: usize) {
        self.buf.reserve(n);
    }

    /// Writes `v` (its [`Snap::put`]).
    pub fn put<T: Snap>(&mut self, v: &T) {
        v.put(self);
    }

    /// Writes one byte.
    #[inline]
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Writes a `u16`, little-endian.
    #[inline]
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Writes a `u32`, little-endian.
    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Writes a `u64`, little-endian.
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Writes a `u128`, little-endian.
    #[inline]
    pub fn u128(&mut self, v: u128) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Writes `bytes` as they are (no length).
    #[inline]
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Writes `n` zero bytes (padding of fixed-size encodings).
    #[inline]
    pub fn zeros(&mut self, n: usize) {
        self.buf.resize(self.buf.len() + n, 0);
    }

    /// Writes a length or count (unsigned LEB128).
    #[inline]
    pub fn length(&mut self, n: usize) {
        let mut n = n as u64;
        while n >= 0x80 {
            self.buf.push((n as u8) | 0x80);
            n >>= 7;
        }
        self.buf.push(n as u8);
    }

    /// Writes a string: its length, then its UTF-8 bytes.
    #[inline]
    pub fn str(&mut self, s: &str) {
        self.length(s.len());
        self.bytes(s.as_bytes());
    }

    /// The shared index of the allocation at `addr` of type `ty` if it was
    /// written before; otherwise records it under the next index and
    /// returns `None` (the caller then writes it inline).
    pub(crate) fn share(&mut self, addr: usize, ty: TypeId) -> Option<u64> {
        let next = self.shared.len() as u64;
        match self.shared.entry((addr, ty)) {
            Entry::Occupied(e) => Some(*e.get()),
            Entry::Vacant(e) => {
                e.insert(next);
                None
            }
        }
    }

    /// A writer in the same mode with an empty buffer and sharing table:
    /// canonical mode encodes each map entry alone to sort by its bytes.
    pub(crate) fn scratch(&self) -> Writer {
        Writer { canonical: self.canonical, ..Self::new() }
    }
}

/// Reads encoded values from a byte slice, or from a stream
/// ([`Reader::streaming`]). Every read is bounds-checked; no read panics.
pub struct Reader<'a> {
    /// The input of a slice reader; empty for a streaming one.
    buf: &'a [u8],
    /// The next byte, in [`Self::data`].
    pos: usize,
    /// Absolute offset of `data()[0]` (a sub-reader of a fixed-size block,
    /// or the window of a streaming reader).
    base: usize,
    /// Shared (`Rc`/`Arc`) objects by index: `None` while being decoded.
    shared: Vec<Option<Box<dyn Any>>>,
    /// Whether this is a streaming reader (spec/snap.md "Streaming
    /// reads"): the bytes not yet read stay in `src`, the window `win`
    /// holds at most [`WINDOW`] of them beyond the longest single read,
    /// and bytes already decoded are dropped, so decoding needs no copy of
    /// the whole input. Kept inline (not boxed) so that a read's choice
    /// between `buf` and `win` needs no extra load.
    streaming: bool,
    /// A streaming reader's window: bytes `base..base + win.len()`.
    win: Vec<u8>,
    /// A streaming reader's source.
    src: Option<Box<dyn std::io::Read + 'a>>,
    /// Input bytes still in `src`.
    left: usize,
    /// The largest id read in each id space ([`Reader::note_id`]).
    ids: [u32; ID_SPACES],
}

/// The bytes a streaming reader reads from its source at a time.
pub const WINDOW: usize = 1 << 20;

/// The number of id spaces a [`Reader`] tracks ([`Reader::note_id`]).
pub const ID_SPACES: usize = 32;

impl fmt::Debug for Reader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reader")
            .field("offset", &self.offset())
            .field("remaining", &self.remaining())
            .field("shared", &self.shared.len())
            .field("streaming", &self.streaming)
            .finish()
    }
}

impl<'a> Reader<'a> {
    /// A reader over `bytes`, at offset 0.
    pub fn new(bytes: &'a [u8]) -> Self {
        Reader::sub(bytes, 0)
    }

    /// Records that an id with index `index` of id space `space` was read
    /// (spec/snap.md "Id ranges"): an id type's `get` calls it, and the
    /// root that owns the space compares [`Reader::id_max`] with the arena
    /// it indexes once it is decoded. A `space` of [`ID_SPACES`] or more is
    /// ignored.
    #[inline]
    pub fn note_id(&mut self, space: usize, index: u32) {
        if let Some(m) = self.ids.get_mut(space) {
            *m = (*m).max(index);
        }
    }

    /// The largest index [`Reader::note_id`] recorded for `space` so far
    /// (0 when none).
    pub fn id_max(&self, space: usize) -> u32 {
        self.ids.get(space).copied().unwrap_or(0)
    }

    /// A slice reader whose first byte has the absolute offset `base`.
    #[inline]
    fn sub(bytes: &'a [u8], base: usize) -> Self {
        Reader {
            buf: bytes,
            pos: 0,
            base,
            shared: Vec::new(),
            streaming: false,
            win: Vec::new(),
            src: None,
            left: 0,
            ids: [0; ID_SPACES],
        }
    }

    /// A reader of the next `len` bytes of `src`, whose first byte has the
    /// absolute offset `base` (for errors). The bytes are read [`WINDOW`]
    /// at a time and dropped once decoded; a source that ends early, or
    /// fails, is [`ErrorKind::Io`] at the offset it was needed.
    pub fn streaming(src: impl std::io::Read + 'a, len: usize, base: usize) -> Self {
        Reader {
            buf: &[],
            pos: 0,
            base,
            shared: Vec::new(),
            streaming: true,
            win: Vec::new(),
            src: Some(Box::new(src)),
            left: len,
            ids: [0; ID_SPACES],
        }
    }

    /// The bytes in hand: the input of a slice reader, a streaming
    /// reader's window.
    #[inline]
    fn data(&self) -> &[u8] {
        if self.streaming { &self.win } else { self.buf }
    }

    /// Makes `n` bytes available at `pos`, reading from a streaming
    /// reader's source; `Ok(false)` when the input has fewer left.
    #[inline]
    fn ensure(&mut self, n: usize) -> Result<bool, SnapError> {
        if self.data().len() - self.pos >= n {
            return Ok(true);
        }
        self.refill(n)
    }

    #[cold]
    #[inline(never)]
    fn refill(&mut self, n: usize) -> Result<bool, SnapError> {
        let at = self.offset();
        let pos = self.pos;
        let have = self.data().len() - pos;
        let Some(src) = self.src.as_deref_mut() else { return Ok(false) };
        if have + self.left < n {
            return Ok(false);
        }
        let win = &mut self.win;
        // Drop the decoded bytes; keep the window's allocation unless one
        // long read grew it.
        win.drain(..pos);
        if win.capacity() > 4 * WINDOW && n <= WINDOW {
            win.shrink_to(WINDOW);
        }
        let want = (n - have).max(WINDOW.min(self.left)).min(self.left);
        // `read_to_end` appends into the spare capacity (no zero fill).
        win.reserve(want);
        let old = win.len();
        let got = std::io::Read::read_to_end(&mut std::io::Read::take(&mut *src, want as u64), win);
        if let Err(e) = got.and_then(|got| {
            if got == want {
                Ok(())
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
            }
        }) {
            win.truncate(old);
            self.left = 0;
            self.src = None;
            self.base += pos;
            self.pos = 0;
            return Err(SnapError::new(ErrorKind::Io(e.to_string()), "snapshot", at));
        }
        self.left -= want;
        self.base += pos;
        self.pos = 0;
        Ok(true)
    }

    /// The absolute offset of the next byte.
    pub fn offset(&self) -> usize {
        self.base + self.pos
    }

    /// Bytes left.
    pub fn remaining(&self) -> usize {
        self.data().len() - self.pos + self.left
    }

    /// Reads `v` (its [`Snap::get`]).
    pub fn get<T: Snap>(&mut self) -> Result<T, SnapError> {
        T::get(self)
    }

    /// Fails unless every byte was read.
    pub fn finish(self) -> Result<(), SnapError> {
        match self.remaining() {
            0 => Ok(()),
            n => Err(SnapError::new(ErrorKind::TrailingBytes(n), "snapshot", self.offset())),
        }
    }

    /// An error of `kind` while reading a `T`, at the current offset.
    #[cold]
    #[inline(never)]
    pub fn error<T: ?Sized>(&self, kind: ErrorKind) -> SnapError {
        SnapError::new(kind, type_name::<T>(), self.offset())
    }

    /// [`ErrorKind::Invalid`] with `why`, for hand-written decoders.
    #[cold]
    #[inline(never)]
    pub fn invalid<T: ?Sized>(&self, why: impl Into<String>) -> SnapError {
        self.error::<T>(ErrorKind::Invalid(why.into()))
    }

    /// [`ErrorKind::BadTag`] for a `width`-byte tag just read.
    #[cold]
    #[inline(never)]
    pub fn bad_tag<T: ?Sized>(&self, tag: u64, width: usize) -> SnapError {
        SnapError::new(ErrorKind::BadTag(tag), type_name::<T>(), self.offset() - width)
    }

    #[cold]
    #[inline(never)]
    fn truncated(&self, n: usize, what: &'static str) -> SnapError {
        SnapError::new(ErrorKind::Truncated { needed: n - self.remaining() }, what, self.offset())
    }

    /// The next `N` bytes.
    #[inline]
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], SnapError> {
        self.array_of("bytes")
    }

    /// The next `N` bytes, as the encoding of a `ty` (for errors).
    #[inline]
    pub(crate) fn array_of<const N: usize>(
        &mut self,
        ty: &'static str,
    ) -> Result<[u8; N], SnapError> {
        if let Some(a) = self.data().get(self.pos..).and_then(<[u8]>::first_chunk::<N>) {
            let a = *a;
            self.pos += N;
            return Ok(a);
        }
        if self.ensure(N)? {
            return self.array_of(ty);
        }
        Err(self.truncated(N, ty))
    }

    /// Reads one byte.
    #[inline]
    pub fn u8(&mut self) -> Result<u8, SnapError> {
        if let Some(&b) = self.data().get(self.pos) {
            self.pos += 1;
            return Ok(b);
        }
        if self.ensure(1)? {
            return self.u8();
        }
        Err(self.truncated(1, "u8"))
    }

    /// Reads a little-endian `u16`.
    #[inline]
    pub fn u16(&mut self) -> Result<u16, SnapError> {
        self.array_of("u16").map(u16::from_le_bytes)
    }

    /// Reads a little-endian `u32`.
    #[inline]
    pub fn u32(&mut self) -> Result<u32, SnapError> {
        self.array_of("u32").map(u32::from_le_bytes)
    }

    /// Reads a little-endian `u64`.
    #[inline]
    pub fn u64(&mut self) -> Result<u64, SnapError> {
        self.array_of("u64").map(u64::from_le_bytes)
    }

    /// Reads a little-endian `u128`.
    #[inline]
    pub fn u128(&mut self) -> Result<u128, SnapError> {
        self.array_of("u128").map(u128::from_le_bytes)
    }

    /// The next `n` bytes, borrowed from the reader (valid until its next
    /// read).
    #[inline]
    pub fn bytes(&mut self, n: usize) -> Result<&[u8], SnapError> {
        if !self.ensure(n)? {
            return Err(self.truncated(n, "bytes"));
        }
        let at = self.pos;
        self.pos += n;
        Ok(&self.data()[at..at + n])
    }

    /// Skips `n` bytes (padding of fixed-size encodings).
    #[inline]
    pub fn skip(&mut self, n: usize) -> Result<(), SnapError> {
        self.bytes(n).map(|_| ())
    }

    /// Skips `n` padding bytes written by [`Writer::zeros`], checking that
    /// each is zero: a nonzero pad byte is damage (or a mismatched layout)
    /// that a plain [`skip`](Reader::skip) would hide.
    #[inline]
    pub fn skip_zeros(&mut self, n: usize) -> Result<(), SnapError> {
        let at = self.offset();
        let b = self.bytes(n)?;
        match b.iter().position(|&x| x != 0) {
            None => Ok(()),
            Some(i) => {
                Err(SnapError::new(ErrorKind::BadValue("nonzero padding"), "padding", at + i))
            }
        }
    }

    /// Reads a length or count written by [`Writer::length`]. Not checked
    /// against the input: use [`Reader::seq_len`] for element counts.
    #[inline]
    pub fn length(&mut self) -> Result<usize, SnapError> {
        let at = self.offset();
        let n = self.leb128()?;
        usize::try_from(n).map_err(|_| SnapError::new(ErrorKind::LengthTooLarge(n), "length", at))
    }

    #[inline]
    fn leb128(&mut self) -> Result<u64, SnapError> {
        let at = self.offset();
        let mut v = 0u64;
        let mut shift = 0u32;
        loop {
            let b = self.u8()?;
            if (shift == 63 && b > 1) || shift > 63 {
                return Err(SnapError::new(ErrorKind::BadValue("LEB128 length"), "length", at));
            }
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }

    /// Reads a string written by [`Writer::str`], borrowed from the reader
    /// (valid until its next read).
    #[inline]
    pub fn str(&mut self) -> Result<&str, SnapError> {
        let n = self.seq_len::<u8>()?;
        let at = self.offset();
        let b = self.bytes(n)?;
        std::str::from_utf8(b).map_err(|_| SnapError::new(ErrorKind::BadValue("UTF-8"), "str", at))
    }

    /// Reads an element count for a sequence of `T` and checks it against
    /// the input left: each element of a fixed [`Snap::SIZE`] `s > 0`
    /// needs `s` bytes, any other at least one (spec/snap.md "Errors");
    /// zero-size elements are capped at `u32::MAX`. A count that passes
    /// is safe to pre-allocate.
    #[inline]
    pub fn seq_len<T: Snap>(&mut self) -> Result<usize, SnapError> {
        let at = self.offset();
        let n = self.leb128()?;
        let fits = match T::SIZE {
            Some(0) => u32::try_from(n).is_ok(),
            Some(s) => n.checked_mul(s as u64).is_some_and(|b| b <= self.remaining() as u64),
            None => n <= self.remaining() as u64,
        };
        if !fits {
            return Err(SnapError::new(ErrorKind::LengthTooLarge(n), type_name::<T>(), at));
        }
        Ok(n as usize)
    }

    /// Reads `n` elements of `T` (a count from [`Reader::seq_len`]) and
    /// hands each to `push`, in order.
    ///
    /// The fixed-size fast path: when `T::SIZE` is `Some(s)`, the bytes
    /// are taken a block at a time (the whole `n * s` from a slice reader,
    /// at most a [`WINDOW`] from a streaming one) with one bounds check,
    /// and each element is decoded from its own `s`-byte chunk
    /// (`chunks_exact`).
    #[inline]
    pub fn seq_items<T: Snap>(
        &mut self,
        n: usize,
        mut push: impl FnMut(T),
    ) -> Result<(), SnapError> {
        match T::SIZE {
            Some(s) if s > 0 => {
                if n.checked_mul(s).is_none() {
                    return Err(self.error::<T>(ErrorKind::LengthTooLarge(n as u64)));
                }
                let per = if self.streaming { (WINDOW / s).max(1) } else { n.max(1) };
                let mut left = n;
                let mut ids = self.ids;
                while left > 0 {
                    let k = left.min(per);
                    let start = self.offset();
                    let block = self.bytes(k * s)?;
                    // One sub-reader, moved from chunk to chunk, carries the
                    // id maxima through the block and back.
                    let mut sub = Reader::sub(&[], start);
                    sub.ids = ids;
                    for (i, chunk) in block.chunks_exact(s).enumerate() {
                        sub.buf = chunk;
                        sub.pos = 0;
                        sub.base = start + i * s;
                        push(T::get(&mut sub)?);
                        debug_assert_eq!(
                            sub.pos,
                            s,
                            "{}::get read {} bytes of its SIZE {s}",
                            type_name::<T>(),
                            sub.pos
                        );
                    }
                    ids = sub.ids;
                    left -= k;
                }
                self.ids = ids;
            }
            _ => {
                for _ in 0..n {
                    push(T::get(self)?);
                }
            }
        }
        Ok(())
    }

    /// The shared object at `index`, cloned (an `Rc`/`Arc` clone).
    pub(crate) fn shared<P: Clone + 'static>(&self, index: u64) -> Result<P, SnapError> {
        let entry = usize::try_from(index).ok().and_then(|i| self.shared.get(i));
        match entry {
            None => Err(self.error::<P>(ErrorKind::SharedOutOfRange(index))),
            Some(None) => Err(self.error::<P>(ErrorKind::SharedCycle)),
            Some(Some(b)) => b
                .downcast_ref::<P>()
                .cloned()
                .ok_or_else(|| self.error::<P>(ErrorKind::SharedWrongType)),
        }
    }

    /// Reserves the next shared index (pre-order, as the writer numbers).
    pub(crate) fn shared_reserve(&mut self) -> usize {
        self.shared.push(None);
        self.shared.len() - 1
    }

    /// Fills a reserved shared index.
    pub(crate) fn shared_fill(&mut self, slot: usize, v: Box<dyn Any>) {
        self.shared[slot] = Some(v);
    }
}
