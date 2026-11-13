//! Front-end state snapshots: the [`Snap`] trait, its byte-level runtime
//! ([`Writer`], [`Reader`]), the standard-library impls and the
//! [schema hash](schema_hash). `#[derive(Snap)]` comes from
//! `kiln_snap_derive` and is re-exported here.
//!
//! See `spec/snap.md` for the contract this crate implements. In short:
//! a snapshot is the state of the front end written as data and read back
//! into fresh objects with every id unchanged. Both ends are the same kiln
//! build (checked with [`schema_hash`]), so the format is not stable
//! across builds and needs no versioning of its own.
//!
//! - Decoding never panics and never trusts a length: every failure is a
//!   [`SnapError`] carrying the type, the byte offset and the field path.
//! - Types whose encoding has a constant size ([`Snap::SIZE`]) are read in
//!   bulk from sequences: one bounds check per collection.
//! - `Rc`/`Arc` allocations are written once and shared again on read.
#![forbid(unsafe_code)]

// The derive names this crate `::kiln_snap`; this makes that path resolve
// inside the crate's own tests.
extern crate self as kiln_snap;

mod error;
mod impls;
mod schema;
mod wire;

pub use error::{ErrorKind, SnapError};
pub use kiln_snap_derive::Snap;
pub use schema::{Desc, Schema, schema_hash};
pub use wire::{Reader, Writer};

/// A type that can be written to a snapshot and read back.
///
/// Implement it with `#[derive(Snap)]` (spec/snap.md "The derive"), or by
/// hand for a root that holds locks, caches or references (spec/snap.md
/// "Hand-written roots"). Only [`put`](Snap::put), [`get`](Snap::get) and
/// [`schema`](Snap::schema) are required.
///
/// Contract:
/// - `get` reads exactly the bytes `put` wrote and returns an equal value;
///   it never panics on any input.
/// - If [`SIZE`](Snap::SIZE) is `Some(n)`, every `put` writes exactly `n`
///   bytes and the encoding holds no shared (`Rc`/`Arc`) object. If it is
///   `None`, every `put` writes at least one byte.
/// - `schema` registers the type's descriptor once and recurses into the
///   types it encodes (see [`Schema::register`]).
pub trait Snap: Sized + 'static {
    /// The constant encoded size in bytes, if every value has one.
    /// Sequences of such types take the bulk path
    /// ([`Reader::seq_items`]).
    const SIZE: Option<usize> = None;

    /// The constant encoded size of `Option<Self>`. The default is one tag
    /// byte plus [`SIZE`](Snap::SIZE) (a `None` is padded with zeros);
    /// types with a niche (`NonZero*`, and derived newtypes of them)
    /// override it together with [`put_option`](Snap::put_option) and
    /// [`get_option`](Snap::get_option).
    const OPTION_SIZE: Option<usize> = match Self::SIZE {
        Some(n) => Some(n + 1),
        None => None,
    };

    /// Writes `self`.
    fn put(&self, w: &mut Writer);

    /// Reads a value written by [`put`](Snap::put).
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError>;

    /// Registers the type's descriptor in `s` (and, the first time, those
    /// of the types it encodes).
    fn schema(s: &mut Schema);

    /// Writes `Option<Self>` (see [`OPTION_SIZE`](Snap::OPTION_SIZE)).
    #[inline]
    fn put_option(v: Option<&Self>, w: &mut Writer) {
        match v {
            None => {
                w.u8(0);
                if let Some(n) = Self::SIZE {
                    w.zeros(n);
                }
            }
            Some(x) => {
                w.u8(1);
                x.put(w);
            }
        }
    }

    /// Reads `Option<Self>` written by [`put_option`](Snap::put_option).
    #[inline]
    fn get_option(r: &mut Reader<'_>) -> Result<Option<Self>, SnapError> {
        match r.u8()? {
            0 => {
                if let Some(n) = Self::SIZE {
                    r.skip(n)?;
                }
                Ok(None)
            }
            1 => Self::get(r).map(Some),
            t => Err(r.bad_tag::<Option<Self>>(u64::from(t), 1)),
        }
    }
}

/// Encodes `v` (normal mode: hash maps in iteration order).
pub fn to_bytes<T: Snap>(v: &T) -> Vec<u8> {
    let mut w = Writer::new();
    v.put(&mut w);
    w.into_bytes()
}

/// Encodes `v` in canonical mode ([`Writer::canonical`]): equal states
/// give equal bytes.
pub fn to_bytes_canonical<T: Snap>(v: &T) -> Vec<u8> {
    let mut w = Writer::canonical();
    v.put(&mut w);
    w.into_bytes()
}

/// Decodes a `T` from all of `bytes`.
///
/// # Errors
/// Any [`SnapError`]; [`ErrorKind::TrailingBytes`] if bytes are left.
pub fn from_bytes<T: Snap>(bytes: &[u8]) -> Result<T, SnapError> {
    let mut r = Reader::new(bytes);
    let v = T::get(&mut r)?;
    r.finish()?;
    Ok(v)
}

/// Writes a sequence of `len` items: the count, then each item.
/// Hand-written impls of collections use it with [`Reader::seq_len`] and
/// [`Reader::seq_items`].
#[inline]
pub fn put_seq<'x, T: Snap>(w: &mut Writer, len: usize, items: impl IntoIterator<Item = &'x T>) {
    w.length(len);
    if let Some(s) = T::SIZE {
        w.reserve(len.saturating_mul(s));
    }
    let start = w.position();
    let mut count = 0usize;
    for x in items {
        x.put(w);
        count += 1;
    }
    debug_assert_eq!(count, len, "put_seq: {len} items announced, {count} written");
    if let Some(s) = T::SIZE {
        debug_assert_eq!(
            w.position() - start,
            len * s,
            "{} wrote other than its SIZE {s}",
            std::any::type_name::<T>()
        );
    }
}

/// What `#[derive(Snap)]` rejects, each with a `compile_error!` at the
/// offending token (spec/snap.md "The derive"). Doc tests only.
///
/// A lifetime parameter (a snapshot owns its data):
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// struct S<'a>(&'a u8);
/// ```
/// A union:
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// union U { a: u8 }
/// ```
/// An unknown or misplaced attribute:
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// struct S { #[snap(rebuild)] a: u8 }
/// ```
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// #[snap(skip)]
/// struct S { a: u8 }
/// ```
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// enum E { #[snap(skip)] A }
/// ```
/// Both `skip` and `with`:
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// struct S { #[snap(skip, with = "m")] a: u8 }
/// ```
/// A field type without `Snap`:
/// ```compile_fail
/// #[derive(kiln_snap::Snap)]
/// struct S { a: std::fs::File }
/// ```
/// The control (compiles):
/// ```
/// #[derive(kiln_snap::Snap)]
/// struct S<T, const N: usize> { #[snap(skip)] a: Vec<T>, b: [u8; N] }
/// ```
#[doc(hidden)]
pub mod derive_errors {}

/// Support for the derive's generated code; not part of the interface.
#[doc(hidden)]
pub mod __private {
    use crate::{Reader, SnapError, Writer};

    /// The size of a field list: the sum, or `None` if any is `None`.
    pub const fn sum(sizes: &[Option<usize>]) -> Option<usize> {
        let mut total = 0;
        let mut i = 0;
        while i < sizes.len() {
            match sizes[i] {
                Some(n) => total += n,
                None => return None,
            }
            i += 1;
        }
        Some(total)
    }

    /// The size of an enum: the tag plus its largest variant (smaller
    /// variants are zero-padded), or `None` if any variant is `None`.
    pub const fn enum_size(tag: usize, variants: &[Option<usize>]) -> Option<usize> {
        let mut max = 0;
        let mut i = 0;
        while i < variants.len() {
            match variants[i] {
                Some(n) => {
                    if n > max {
                        max = n;
                    }
                }
                None => return None,
            }
            i += 1;
        }
        Some(tag + max)
    }

    /// Pads a fixed-size enum's variant to the enum's size.
    #[inline]
    pub fn pad(w: &mut Writer, total: Option<usize>, tag: usize, variant: Option<usize>) {
        if let (Some(t), Some(v)) = (total, variant) {
            w.zeros(t - tag - v);
        }
    }

    /// Skips the padding [`pad`] wrote.
    #[inline]
    pub fn skip_pad(
        r: &mut Reader<'_>,
        total: Option<usize>,
        tag: usize,
        variant: Option<usize>,
    ) -> Result<(), SnapError> {
        match (total, variant) {
            (Some(t), Some(v)) => r.skip(t - tag - v),
            _ => Ok(()),
        }
    }

    /// Adds the field `at` to an error's path.
    #[inline]
    pub fn field<T>(res: Result<T, SnapError>, at: &'static str) -> Result<T, SnapError> {
        match res {
            Ok(v) => Ok(v),
            Err(e) => Err(e.within(at)),
        }
    }
}
