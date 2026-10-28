//! Compact location types: `FileId`, `Loc`, `Span`.

use std::fmt;

/// Bit set on every macro-expansion location. File locations live in
/// `[1, MACRO_BIT)`, expansion locations in `[MACRO_BIT, 2^32)`.
pub(crate) const MACRO_BIT: u32 = 1 << 31;

/// Identifies a buffer owned by a [`SourceManager`](crate::SourceManager):
/// a file loaded from disk or a virtual buffer (predefines, `-D` options,
/// token-paste results, ...).
///
/// Ids are dense indices in creation order. A `FileId` is only meaningful
/// for the `SourceManager` that created it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct FileId(pub(crate) u32);

impl FileId {
    /// The dense index of this buffer (0 for the first buffer created).
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A compact source location (4 bytes).
///
/// The `u32` value space is partitioned by the
/// [`SourceManager`](crate::SourceManager):
/// - `0` is [`Loc::INVALID`];
/// - `[1, 2^31)` holds *file locations*: each buffer owns the contiguous
///   range `[start, start + len]` (one extra position for end-of-file), so
///   `start + n` is byte offset `n` of that buffer;
/// - `[2^31, 2^32)` holds *macro-expansion locations*: each expansion entry
///   owns `[base, base + len]`, and `base + n` is spelled at the entry's
///   spelling location plus `n`.
///
/// Locations order by their raw value: within one buffer (or one expansion
/// entry) that is source order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Loc(pub(crate) u32);

impl Loc {
    /// The invalid location (no source position).
    pub const INVALID: Loc = Loc(0);

    /// True for every location except [`Loc::INVALID`].
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// True if this location lies in a macro-expansion range. O(1).
    pub const fn is_macro(self) -> bool {
        self.0 & MACRO_BIT != 0
    }

    /// True if this is a valid location in a buffer (not a macro location).
    pub const fn is_file(self) -> bool {
        self.is_valid() && !self.is_macro()
    }

    /// The location `n` bytes after `self` within the same buffer or
    /// expansion entry. The caller keeps the result inside the entry that
    /// `self` belongs to (at most one past its last byte); locations
    /// outside it map to a different entry.
    #[must_use]
    pub const fn with_offset(self, n: u32) -> Loc {
        Loc(self.0 + n)
    }
}

impl fmt::Debug for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.is_valid() {
            f.write_str("Loc(invalid)")
        } else if self.is_macro() {
            write!(f, "Loc(macro+{})", self.0 & !MACRO_BIT)
        } else {
            write!(f, "Loc({})", self.0)
        }
    }
}

/// A half-open byte range `[lo, hi)` (8 bytes).
///
/// Both ends lie in the same buffer or the same expansion entry for spans
/// produced by the lexer; spans covering a macro invocation (expansion
/// ranges) may have ends in different entries.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct Span {
    /// First byte of the range.
    pub lo: Loc,
    /// One past the last byte of the range.
    pub hi: Loc,
}

impl Span {
    /// The span with both ends invalid.
    pub const INVALID: Span = Span { lo: Loc::INVALID, hi: Loc::INVALID };

    /// Creates `[lo, hi)`.
    pub const fn new(lo: Loc, hi: Loc) -> Span {
        Span { lo, hi }
    }

    /// The empty span at `at`.
    pub const fn empty(at: Loc) -> Span {
        Span { lo: at, hi: at }
    }

    /// True if `lo` is a valid location.
    pub const fn is_valid(self) -> bool {
        self.lo.is_valid()
    }
}

const _: () = assert!(size_of::<Loc>() == 4);
const _: () = assert!(size_of::<Span>() == 8);
const _: () = assert!(size_of::<FileId>() == 4);
