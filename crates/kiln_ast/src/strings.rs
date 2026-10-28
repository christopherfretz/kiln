//! String literal objects: the [`StringPool`] (spec/ast.md §9.1).
//!
//! A string literal's code units are interned by `(encoding, bytes)`, so
//! `"abc"` written twice is stored once and both `StringLiteral` exprs get
//! the same [`StringLitId`]. This is **storage dedup only**: whether two
//! evaluations of string literals yield the same object is unspecified
//! ([lex.string]/p9), and the Oven must not conclude `"a" == "a"` from equal
//! ids — comparing pointers into distinct literal *evaluations* is not a
//! core constant expression ([expr.const]).
//!
//! The stored bytes are the code units **without** the implicit terminating
//! null; [`StringPool::object_bytes`] is the size of the array object
//! (units + 1) × unit size.

use std::hash::Hasher;

use hashbrown::HashTable;
use kiln_source::fx::FxHasher;

use crate::consts::ConstError;
use crate::ids::StringLitId;
use crate::mem::{HeapSize, MemReport, MemRow};

/// The encoding (and code-unit width) of a string literal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum StringEncoding {
    /// `"..."`: ordinary literal, 1-byte `char` units (execution charset).
    Ordinary = 0,
    /// `u8"..."`: UTF-8, 1-byte `char8_t` units.
    Utf8 = 1,
    /// `u"..."`: UTF-16, 2-byte `char16_t` units.
    Utf16 = 2,
    /// `U"..."`: UTF-32, 4-byte `char32_t` units.
    Utf32 = 3,
    /// `L"..."` with a 2-byte `wchar_t` (Windows).
    Wide16 = 4,
    /// `L"..."` with a 4-byte `wchar_t`.
    Wide32 = 5,
}

impl StringEncoding {
    /// Bytes per code unit.
    pub const fn unit_bytes(self) -> u32 {
        match self {
            StringEncoding::Ordinary | StringEncoding::Utf8 => 1,
            StringEncoding::Utf16 | StringEncoding::Wide16 => 2,
            StringEncoding::Utf32 | StringEncoding::Wide32 => 4,
        }
    }

    /// The literal's source prefix (`""`, `"u8"`, `"u"`, `"U"`, `"L"`).
    pub const fn prefix(self) -> &'static str {
        match self {
            StringEncoding::Ordinary => "",
            StringEncoding::Utf8 => "u8",
            StringEncoding::Utf16 => "u",
            StringEncoding::Utf32 => "U",
            StringEncoding::Wide16 | StringEncoding::Wide32 => "L",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    start: u32,
    len: u32,
    enc: StringEncoding,
}

/// Interned string literal code units (append-only).
#[derive(Debug, Default)]
pub struct StringPool {
    bytes: Vec<u8>,
    entries: Vec<Entry>,
    table: HashTable<StringLitId>,
}

fn hash(enc: StringEncoding, bytes: &[u8]) -> u64 {
    let mut h = FxHasher::default();
    h.write_u8(enc as u8);
    h.write_usize(bytes.len());
    h.write(bytes);
    h.finish().rotate_left(26)
}

impl StringPool {
    /// An empty pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns a literal's code units (little-endian bytes, `unit_bytes()`
    /// per unit, without the terminating null); equal `(encoding, bytes)`
    /// return the same id.
    ///
    /// # Panics
    /// If `code_units.len()` is not a multiple of the unit width, or the
    /// pool (4 GiB, 2^32 − 1 literals) is exhausted. Use
    /// [`StringPool::try_intern`] to handle these.
    pub fn intern(&mut self, encoding: StringEncoding, code_units: &[u8]) -> StringLitId {
        match self.try_intern(encoding, code_units) {
            Ok(id) => id,
            Err(e) => panic!("StringPool::intern: {e}"),
        }
    }

    /// [`StringPool::intern`], reporting errors.
    ///
    /// # Errors
    /// [`ConstError::Malformed`] on a partial code unit;
    /// [`ConstError::Capacity`] on pool exhaustion.
    pub fn try_intern(
        &mut self,
        encoding: StringEncoding,
        code_units: &[u8],
    ) -> Result<StringLitId, ConstError> {
        if !code_units.len().is_multiple_of(encoding.unit_bytes() as usize) {
            return Err(ConstError::Malformed("string literal bytes are not whole code units"));
        }
        let h = hash(encoding, code_units);
        let (bytes, entries) = (&self.bytes, &self.entries);
        let eq = |&id: &StringLitId| {
            let e = entries[id.index() as usize - 1];
            e.enc == encoding
                && &bytes[e.start as usize..e.start as usize + e.len as usize] == code_units
        };
        if let Some(&id) = self.table.find(h, eq) {
            return Ok(id);
        }
        let cap = |n: usize| u32::try_from(n).map_err(|_| ConstError::Capacity("string pool"));
        let start = cap(self.bytes.len())?;
        cap(self.bytes.len() + code_units.len())?;
        let id = StringLitId::try_new(cap(self.entries.len() + 1)?)
            .ok_or(ConstError::Capacity("StringLitId"))?;
        self.bytes.extend_from_slice(code_units);
        self.entries.push(Entry { start, len: code_units.len() as u32, enc: encoding });
        let (bytes, entries) = (&self.bytes, &self.entries);
        self.table.insert_unique(h, id, |&id| {
            let e = entries[id.index() as usize - 1];
            hash(e.enc, &bytes[e.start as usize..e.start as usize + e.len as usize])
        });
        Ok(id)
    }

    /// The literal's encoding and code-unit bytes.
    ///
    /// # Panics
    /// If `id` is not from this pool.
    pub fn get(&self, id: StringLitId) -> (StringEncoding, &[u8]) {
        let e = self.entries[id.index() as usize - 1];
        (e.enc, &self.bytes[e.start as usize..e.start as usize + e.len as usize])
    }

    /// The code-unit bytes.
    pub fn bytes(&self, id: StringLitId) -> &[u8] {
        self.get(id).1
    }

    /// The encoding.
    pub fn encoding(&self, id: StringLitId) -> StringEncoding {
        self.get(id).0
    }

    /// Number of code units, excluding the terminator.
    pub fn unit_count(&self, id: StringLitId) -> u64 {
        let (enc, b) = self.get(id);
        (b.len() / enc.unit_bytes() as usize) as u64
    }

    /// Code unit `i` (`i == unit_count` is the terminating 0); `None` past
    /// the end of the array object.
    pub fn unit(&self, id: StringLitId, i: u64) -> Option<u32> {
        let (enc, b) = self.get(id);
        let w = enc.unit_bytes() as usize;
        let n = (b.len() / w) as u64;
        if i == n {
            return Some(0);
        }
        if i > n {
            return None;
        }
        let at = i as usize * w;
        Some(b[at..at + w].iter().rev().fold(0u32, |acc, &x| (acc << 8) | u32::from(x)))
    }

    /// Size in bytes of the literal's array object (including the null
    /// terminator).
    pub fn object_bytes(&self, id: StringLitId) -> u64 {
        let enc = self.encoding(id);
        (self.unit_count(id) + 1) * u64::from(enc.unit_bytes())
    }

    /// Number of distinct literals.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if no literal has been interned.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Appends the literal in source form: prefix, `"`, the units, `"`.
    /// `\\`, `\"`, `\n`, `\t`, `\r`, `\0`, `\a`, `\b`, `\f`, `\v` use their
    /// escapes, printable ASCII prints verbatim, anything else as `\x{hh}`
    /// (lowercase hex of the code unit, no leading zeros).
    pub fn write_literal(&self, id: StringLitId, out: &mut String) {
        let enc = self.encoding(id);
        out.push_str(enc.prefix());
        out.push('"');
        for i in 0..self.unit_count(id) {
            push_unit(self.unit(id, i).unwrap_or(0), '"', out);
        }
        out.push('"');
    }
}

/// Appends code unit `u` as it appears inside a literal delimited by
/// `quote` (`"` or `'`).
pub(crate) fn push_unit(u: u32, quote: char, out: &mut String) {
    use std::fmt::Write as _;
    match u {
        0x5c => out.push_str("\\\\"),
        0x0a => out.push_str("\\n"),
        0x09 => out.push_str("\\t"),
        0x0d => out.push_str("\\r"),
        0x00 => out.push_str("\\0"),
        0x07 => out.push_str("\\a"),
        0x08 => out.push_str("\\b"),
        0x0c => out.push_str("\\f"),
        0x0b => out.push_str("\\v"),
        _ if char::from_u32(u) == Some(quote) => {
            out.push('\\');
            out.push(quote);
        }
        0x20..=0x7e => out.push(char::from(u as u8)),
        _ => {
            let _ = write!(out, "\\x{{{u:x}}}");
        }
    }
}

impl HeapSize for StringPool {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("bytes", &self.bytes));
        r.push(MemRow::of_vec("entries", &self.entries));
        let slot = size_of::<StringLitId>() as u64 + 1;
        r.push(MemRow::new(
            "intern",
            self.table.len() as u64,
            self.table.len() as u64 * slot,
            self.table.allocation_size() as u64,
        ));
        r
    }
}

#[cfg(test)]
#[allow(clippy::many_single_char_names, reason = "test locals")]
mod tests {
    use super::*;

    #[test]
    fn dedup_by_encoding_and_bytes() {
        let mut p = StringPool::new();
        let a = p.intern(StringEncoding::Ordinary, b"hi");
        let b = p.intern(StringEncoding::Utf8, b"hi");
        assert_ne!(a, b);
        assert_eq!(p.intern(StringEncoding::Ordinary, b"hi"), a);
        assert_eq!(p.get(a), (StringEncoding::Ordinary, &b"hi"[..]));
        assert_eq!(p.len(), 2);
        assert_eq!(p.object_bytes(a), 3);
        let w = p.intern(StringEncoding::Utf16, &[0x41, 0, 0xac, 0x20]);
        assert_eq!(p.unit_count(w), 2);
        assert_eq!(p.unit(w, 1), Some(0x20ac));
        assert_eq!(p.unit(w, 2), Some(0));
        assert_eq!(p.unit(w, 3), None);
        assert_eq!(p.object_bytes(w), 6);
        let mut s = String::new();
        p.write_literal(w, &mut s);
        assert_eq!(s, "u\"A\\x{20ac}\"");
        assert!(p.try_intern(StringEncoding::Utf32, b"abc").is_err());
    }

    #[test]
    fn escapes() {
        let mut p = StringPool::new();
        let id = p.intern(StringEncoding::Ordinary, b"a\"\\\n\t\0\x7f'");
        let mut s = String::new();
        p.write_literal(id, &mut s);
        assert_eq!(s, r#""a\"\\\n\t\0\x{7f}'""#);
    }
}
