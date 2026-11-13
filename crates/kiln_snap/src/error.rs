//! [`SnapError`]: why a snapshot could not be decoded.

use std::fmt;

/// What went wrong while decoding (spec/snap.md "Errors").
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The input ended `needed` bytes too early.
    Truncated {
        /// Bytes the read needed beyond the end of the input.
        needed: usize,
    },
    /// An enum or `Option` tag that names no variant.
    BadTag(u64),
    /// A value the type cannot hold: a `bool` other than 0 or 1, a `char`
    /// that is not a scalar value, a zero `NonZero*`, invalid UTF-8, an
    /// integer out of range for `usize`.
    BadValue(&'static str),
    /// A length or count larger than the bytes left could hold.
    LengthTooLarge(u64),
    /// A shared (`Rc`/`Arc`) back-reference to an index not yet written.
    SharedOutOfRange(u64),
    /// A shared back-reference to an object of another type.
    SharedWrongType,
    /// A shared back-reference to an object still being decoded (a cycle,
    /// which no snapshot written by [`Writer`](crate::Writer) contains).
    SharedCycle,
    /// [`from_bytes`](crate::from_bytes) decoded its value with bytes left.
    TrailingBytes(usize),
    /// A hand-written decoder rejected the data (an index out of range, an
    /// inconsistent table).
    Invalid(String),
    /// A streaming reader's source failed or ended early
    /// ([`Reader::streaming`](crate::Reader::streaming)).
    Io(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Truncated { needed } => write!(f, "input truncated ({needed} more bytes)"),
            ErrorKind::BadTag(t) => write!(f, "invalid tag {t}"),
            ErrorKind::BadValue(what) => write!(f, "invalid {what}"),
            ErrorKind::LengthTooLarge(n) => write!(f, "length {n} exceeds the remaining input"),
            ErrorKind::SharedOutOfRange(i) => write!(f, "shared index {i} out of range"),
            ErrorKind::SharedWrongType => f.write_str("shared index of the wrong type"),
            ErrorKind::SharedCycle => f.write_str("shared index refers to an unfinished object"),
            ErrorKind::TrailingBytes(n) => write!(f, "{n} trailing bytes"),
            ErrorKind::Invalid(why) => f.write_str(why),
            ErrorKind::Io(why) => write!(f, "read error: {why}"),
        }
    }
}

/// A decoding failure: the [`ErrorKind`], the name of the type being read,
/// the absolute byte offset, and the path of enclosing fields
/// (`Outer.field`, outermost first). Boxed, so that `Result<T, SnapError>`
/// stays small on the decoding fast path.
#[derive(Clone, PartialEq, Eq)]
pub struct SnapError(Box<Inner>);

#[derive(Clone, PartialEq, Eq)]
struct Inner {
    kind: ErrorKind,
    ty: &'static str,
    offset: usize,
    /// Innermost first (pushed while unwinding).
    path: Vec<&'static str>,
}

impl SnapError {
    /// An error of `kind` while reading a `ty` at byte `offset`.
    #[cold]
    #[inline(never)]
    pub fn new(kind: ErrorKind, ty: &'static str, offset: usize) -> Self {
        SnapError(Box::new(Inner { kind, ty, offset, path: Vec::new() }))
    }

    /// What went wrong.
    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }

    /// The type being read (`std::any::type_name`, or the primitive).
    pub fn type_name(&self) -> &'static str {
        self.0.ty
    }

    /// The absolute byte offset in the input where the failing read
    /// started.
    pub fn offset(&self) -> usize {
        self.0.offset
    }

    /// The enclosing fields, outermost first (`["Outer.inner",
    /// "Inner.count"]`).
    pub fn path(&self) -> Vec<&'static str> {
        self.0.path.iter().rev().copied().collect()
    }

    /// Adds an enclosing field to the path (the derive calls this as the
    /// error unwinds through each field).
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn within(mut self, field: &'static str) -> Self {
        self.0.path.push(field);
        self
    }
}

impl fmt::Display for SnapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "snapshot: {} reading `{}` at byte {}", self.0.kind, self.0.ty, self.0.offset)?;
        if !self.0.path.is_empty() {
            f.write_str(" (in ")?;
            for (i, p) in self.0.path.iter().rev().enumerate() {
                if i > 0 {
                    f.write_str(" > ")?;
                }
                f.write_str(p)?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Debug for SnapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for SnapError {}
