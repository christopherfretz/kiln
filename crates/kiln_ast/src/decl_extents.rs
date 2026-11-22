//! The declaration-extent table (spec/ast.md §19, WP-2090): the source
//! extents of every declaration as written, for tooling clients that
//! rewrite source (remove, move, rename or wrap a declaration).
//!
//! Opt-in, like the written-reference table (§18): off by default, and
//! off every recording site is one branch on [`DeclExtents::is_on`] and
//! nothing is allocated. On, the parser records one [`DeclExtent`] per
//! declaration it parsed, keyed by the declaration; a template shares its
//! pattern's row. Implicit declarations and instantiations have none.

use std::fmt::Write as _;

use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::ids::DeclId;
use crate::mem::MemRow;
use crate::types::TypeNames;

/// What the last token of a declaration is (spec/ast.md §19.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
#[repr(u8)]
pub enum DeclEnd {
    /// Neither a `;` nor a closing brace: a parameter, an enumerator, a
    /// template parameter, or a declaration an error cut short.
    #[default]
    Other,
    /// The `;` that ends the declaration; it is `[full.hi - 1, full.hi)`.
    Semi,
    /// The `}` that closes the body (a function body, the braces of a
    /// namespace, linkage specification or `export` block); it ends at
    /// `full.hi == body.hi`.
    Brace,
}

impl DeclEnd {
    /// The dump's spelling.
    pub const fn spelling(self) -> &'static str {
        match self {
            DeclEnd::Other => "-",
            DeclEnd::Semi => ";",
            DeclEnd::Brace => "}",
        }
    }
}

/// The extents of one declaration as written (spec/ast.md §19.2; 44
/// bytes). Every [`Span`] is `[start of its first token, end of its last
/// token)` in kiln token locations: an end inside a macro expansion is a
/// macro location, mapped by the source manager as for diagnostics.
/// Absent parts are [`Span::INVALID`] / [`Loc::INVALID`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, kiln_snap::Snap)]
pub struct DeclExtent {
    /// The whole declaration: from its first token (an attribute, the
    /// `template` of its template head, a braceless `extern "C"` or
    /// `export`, a decl-specifier, the declarator-id) to its last (see
    /// `end`). Shared by every entity one declaration declares.
    pub full: Span,
    /// The part of the declaration that is this entity's alone: the
    /// init-declarator (declarator, initializer, bit-field width,
    /// pure-specifier, function body) of a declarator, the
    /// class-specifier, enum-specifier or elaborated-type-specifier of a
    /// tag declared in a decl-specifier-seq; `full` otherwise.
    pub own: Span,
    /// The unqualified-id of the declarator-id or the declared name:
    /// `f`, `operator+`, `operator()`, `~T`, `operator int*`,
    /// `operator""_x`, `S<int>` (the template-id of a specialization).
    /// Invalid for an unnamed declaration.
    pub name: Span,
    /// The start of the nested-name-specifier before `name` (a leading
    /// `::` included), or of the enclosing namespace names of a nested
    /// namespace definition; invalid when unqualified. The qualified name
    /// is `[qual, name.hi)`.
    pub qual: Loc,
    /// The body: a function body with its ctor-initializer and a
    /// function-try-block's handlers; the braces of a class, enumeration,
    /// namespace, linkage specification or `export` block; `= default`,
    /// `= delete` (with its reason) or `= 0`. Invalid when none.
    pub body: Span,
    /// The end of the decl-specifier-seq: the shared prefix of a
    /// declarator group is `[full.lo, specs_end)`. Invalid when the
    /// declaration has none.
    pub specs_end: Loc,
    /// What the last token is.
    pub end: DeclEnd,
}

impl DeclExtent {
    /// The `;` ending the declaration, when there is one.
    pub fn semi(&self) -> Option<Span> {
        (self.end == DeclEnd::Semi && self.full.hi.to_raw() > 0)
            .then(|| Span::new(Loc::from_raw(self.full.hi.to_raw() - 1), self.full.hi))
    }

    /// The qualified name `[qual, name.hi)`, or the name when unqualified.
    pub fn qualified_name(&self) -> Span {
        if self.qual.is_valid() { Span::new(self.qual, self.name.hi) } else { self.name }
    }

    /// The shared prefix of a declarator group, `[full.lo, specs_end)`.
    pub fn prefix(&self) -> Option<Span> {
        self.specs_end.is_valid().then(|| Span::new(self.full.lo, self.specs_end))
    }
}

/// A position in the table, for rollback ([`DeclExtents::truncate`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtentMark(u32);

/// The table (spec/ast.md §19). Written into snapshots and carried in
/// module images.
#[derive(Clone, Debug, Default, kiln_snap::Snap)]
pub struct DeclExtents {
    on: bool,
    /// The rows, in recording order.
    rows: Vec<DeclExtent>,
    /// By declaration index: the row + 1, or 0 for none. Grown on demand,
    /// so it ends at the last declaration with a row.
    index: Vec<u32>,
}

impl DeclExtents {
    /// True when extents are recorded.
    #[inline]
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns recording on or off (rows already made stay).
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// The number of rows (a template sharing its pattern's counts once).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True when there are no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    fn row_of(&self, d: DeclId) -> Option<usize> {
        match self.index.get(d.index() as usize) {
            Some(&r) if r != 0 => Some(r as usize - 1),
            _ => None,
        }
    }

    /// The extents of declaration `d`, if it was parsed with recording on.
    pub fn get(&self, d: DeclId) -> Option<&DeclExtent> {
        self.row_of(d).map(|r| &self.rows[r])
    }

    /// Every declaration with extents, in declaration-id order.
    pub fn iter(&self) -> impl Iterator<Item = (DeclId, &DeclExtent)> + '_ {
        self.index.iter().enumerate().filter(|(_, r)| **r != 0).map(|(i, &r)| {
            (DeclId::new(u32::try_from(i).unwrap_or(u32::MAX)), &self.rows[r as usize - 1])
        })
    }

    /// Records `e` as `d`'s extents (replacing earlier ones); nothing
    /// when off.
    pub fn set(&mut self, d: DeclId, e: DeclExtent) {
        if !self.on {
            return;
        }
        if let Some(r) = self.row_of(d) {
            self.rows[r] = e;
            return;
        }
        let i = d.index() as usize;
        if self.index.len() <= i {
            self.index.resize(i + 1, 0);
        }
        self.rows.push(e);
        self.index[i] = u32::try_from(self.rows.len()).expect("extent table exhausted");
    }

    /// `d`'s row, created empty when it has none; `None` when off.
    pub fn entry(&mut self, d: DeclId) -> Option<&mut DeclExtent> {
        if !self.on {
            return None;
        }
        if self.row_of(d).is_none() {
            self.set(d, DeclExtent::default());
        }
        self.get_mut(d)
    }

    /// `alias` shares `d`'s row (a template and its pattern); nothing when
    /// `d` has none or `alias` has its own.
    pub fn share(&mut self, alias: DeclId, d: DeclId) {
        let Some(r) = self.index.get(d.index() as usize).copied().filter(|&r| r != 0) else {
            return;
        };
        let i = alias.index() as usize;
        if self.index.len() <= i {
            self.index.resize(i + 1, 0);
        }
        if self.index[i] == 0 {
            self.index[i] = r;
        }
    }

    /// Drops `d`'s row, for a declaration the parse made and then
    /// discarded (the `void` of `(void)`): the row itself when it is the
    /// last one, else only `d`'s entry.
    pub fn forget(&mut self, d: DeclId) {
        let Some(r) = self.row_of(d) else { return };
        if r + 1 == self.rows.len() {
            self.truncate(ExtentMark(r as u32));
        } else if let Some(x) = self.index.get_mut(d.index() as usize) {
            *x = 0;
        }
    }

    /// `d`'s row, to complete it.
    pub fn get_mut(&mut self, d: DeclId) -> Option<&mut DeclExtent> {
        self.row_of(d).map(|r| &mut self.rows[r])
    }

    /// The table position now.
    pub fn mark(&self) -> ExtentMark {
        ExtentMark(self.rows.len() as u32)
    }

    /// Drops the rows recorded after `m` (a rolled-back parse leaves none
    /// behind).
    pub fn truncate(&mut self, m: ExtentMark) {
        let n = m.0;
        if n as usize >= self.rows.len() {
            return;
        }
        self.rows.truncate(n as usize);
        for r in &mut self.index {
            if *r > n {
                *r = 0;
            }
        }
        while self.index.last() == Some(&0) {
            self.index.pop();
        }
    }

    /// The snapshot check: every index entry names a row, and no entry
    /// is past the decl arena (`decls` declarations, 1-based).
    pub fn check_restored(&self, decls: usize) -> Result<(), &'static str> {
        // Declaration indices are 1-based: `index` may reach `decls + 1`.
        if self.index.len() > decls + 1 {
            return Err("declaration extent index past the decl arena");
        }
        if self.index.iter().any(|&r| r as usize > self.rows.len()) {
            return Err("declaration extent index past its rows");
        }
        Ok(())
    }

    /// The memory row (`decl_extents`), only when recording is on.
    pub fn mem_row(&self) -> Option<MemRow> {
        if !self.on {
            return None;
        }
        let sz = size_of::<DeclExtent>() as u64;
        Some(MemRow::new(
            "decl_extents",
            self.rows.len() as u64,
            self.rows.len() as u64 * sz + self.index.len() as u64 * 4,
            self.rows.capacity() as u64 * sz + self.index.capacity() as u64 * 4,
        ))
    }

    /// The dump (spec/ast.md §19.7): one line per declaration with
    /// extents, in declaration-id order.
    pub fn dump(&self, ast: &crate::Ast, sm: &SourceManager, main: Option<FileId>) -> String {
        let names = crate::diag_names::DiagNames::new(ast, Some(sm), 22).closures_as_lambda();
        let pos = |loc: Loc| -> String {
            match sm.line_col(sm.spelling_loc(loc)) {
                Some(lc) if Some(lc.file) == main => format!("{}:{}", lc.line, lc.col),
                Some(lc) => {
                    let n = sm.file_name(lc.file);
                    let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
                    format!("{base}:{}:{}", lc.line, lc.col)
                }
                None => "?".to_string(),
            }
        };
        let range = |s: Span| -> String { format!("{}-{}", pos(s.lo), pos(s.hi)) };
        let mut out = String::new();
        for (d, e) in self.iter() {
            let n = ast.decls.get(d);
            let mut name = String::new();
            let scoped = n.kind() != crate::decl::DeclKind::ParmVar;
            names.write_decl_name(d, scoped, &mut name);
            if name.is_empty() || name.ends_with("::") {
                name.push_str(if n.kind() == crate::decl::DeclKind::Namespace {
                    "(anonymous namespace)"
                } else {
                    "(anonymous)"
                });
            }
            let name = strip_lambda_dirs(&name);
            let _ =
                write!(out, "{:?} '{name}'  full={} {}", n.kind(), range(e.full), e.end.spelling());
            if e.own != e.full && e.own.lo.is_valid() {
                let _ = write!(out, " own={}", range(e.own));
            }
            if e.name.lo.is_valid() {
                let text = sm.span_text(e.name).unwrap_or("?");
                let _ = write!(out, " name={} '{text}'", range(e.name));
            }
            if e.qual.is_valid() {
                let _ = write!(out, " qual={}", pos(e.qual));
            }
            if e.body.lo.is_valid() {
                let _ = write!(out, " body={}", range(e.body));
            }
            if e.specs_end.is_valid() {
                let _ = write!(out, " specs-end={}", pos(e.specs_end));
            }
            if sm.is_macro_loc(e.full.lo) {
                let _ = write!(out, " exp={}", pos(sm.expansion_loc(e.full.lo)));
            }
            out.push('\n');
        }
        out
    }
}

/// `name` with the directories of each `(lambda at path:L:C)` dropped,
/// so a dump does not depend on where the file is.
fn strip_lambda_dirs(name: &str) -> String {
    const AT: &str = "(lambda at ";
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(i) = rest.find(AT) {
        out.push_str(&rest[..i + AT.len()]);
        rest = &rest[i + AT.len()..];
        let end = rest.find(')').unwrap_or(rest.len());
        let path = &rest[..end];
        let cut = path.rfind(['/', '\\']).map_or(0, |k| k + 1);
        rest = &rest[cut..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(lo: u32, hi: u32) -> DeclExtent {
        DeclExtent {
            full: Span::new(Loc::from_raw(lo), Loc::from_raw(hi)),
            end: DeclEnd::Semi,
            ..DeclExtent::default()
        }
    }

    #[test]
    fn row_is_44_bytes() {
        assert_eq!(size_of::<DeclExtent>(), 44);
    }

    #[test]
    fn off_records_nothing() {
        let mut t = DeclExtents::default();
        assert!(!t.is_on());
        t.set(DeclId::new(3), row(1, 2));
        assert!(t.is_empty());
        assert!(t.get(DeclId::new(3)).is_none());
        assert!(t.mem_row().is_none());
        assert_eq!(t.rows.capacity(), 0);
        assert_eq!(t.index.capacity(), 0);
    }

    #[test]
    fn set_replaces_and_share_aliases() {
        let mut t = DeclExtents::default();
        t.set_on(true);
        let (pattern, templ) = (DeclId::new(4), DeclId::new(9));
        t.set(pattern, row(1, 5));
        t.set(pattern, row(1, 6));
        assert_eq!(t.len(), 1);
        t.share(templ, pattern);
        assert_eq!(t.get(templ), t.get(pattern));
        assert_eq!(t.get(templ).map(|e| e.full.hi), Some(Loc::from_raw(6)));
        // A shared row is completed once for both.
        if let Some(e) = t.get_mut(pattern) {
            e.end = DeclEnd::Brace;
        }
        assert_eq!(t.get(templ).map(|e| e.end), Some(DeclEnd::Brace));
        let ids: Vec<_> = t.iter().map(|(d, _)| d).collect();
        assert_eq!(ids, [pattern, templ]);
        assert_eq!(t.get(row_id(5)), None);
    }

    fn row_id(i: u32) -> DeclId {
        DeclId::new(i)
    }

    #[test]
    fn semi_and_prefix() {
        let mut e = row(10, 20);
        assert_eq!(e.semi(), Some(Span::new(Loc::from_raw(19), Loc::from_raw(20))));
        e.end = DeclEnd::Brace;
        assert_eq!(e.semi(), None);
        assert_eq!(e.prefix(), None);
        e.specs_end = Loc::from_raw(13);
        assert_eq!(e.prefix(), Some(Span::new(Loc::from_raw(10), Loc::from_raw(13))));
        e.name = Span::new(Loc::from_raw(15), Loc::from_raw(16));
        assert_eq!(e.qualified_name(), e.name);
        e.qual = Loc::from_raw(14);
        assert_eq!(e.qualified_name(), Span::new(Loc::from_raw(14), Loc::from_raw(16)));
    }

    #[test]
    fn forget_drops_a_discarded_declaration() {
        let mut t = DeclExtents::default();
        t.set_on(true);
        let (a, b) = (DeclId::new(2), DeclId::new(3));
        t.set(a, row(1, 2));
        t.set(b, row(3, 4));
        // The last row goes with its entry.
        t.forget(b);
        assert_eq!(t.len(), 1);
        assert!(t.get(b).is_none());
        t.set(b, row(3, 4));
        // An earlier row stays, unreachable.
        t.forget(a);
        assert!(t.get(a).is_none());
        assert_eq!(t.get(b).map(|e| e.full.hi), Some(Loc::from_raw(4)));
        assert_eq!(t.iter().count(), 1);
        t.forget(a);
    }

    #[test]
    fn lambda_names_lose_their_directories() {
        assert_eq!(strip_lambda_dirs("(lambda at /a/b/t.cpp:1:2)::w"), "(lambda at t.cpp:1:2)::w");
        assert_eq!(strip_lambda_dirs("N::f"), "N::f");
    }

    #[test]
    fn truncate_drops_later_rows_and_their_aliases() {
        let mut t = DeclExtents::default();
        t.set_on(true);
        t.set(row_id(2), row(1, 2));
        let m = t.mark();
        t.set(row_id(7), row(3, 4));
        t.share(row_id(8), row_id(7));
        t.truncate(m);
        assert_eq!(t.len(), 1);
        assert!(t.get(row_id(7)).is_none() && t.get(row_id(8)).is_none());
        assert!(t.get(row_id(2)).is_some());
        // Declaration 2 needs an arena of at least two declarations.
        assert_eq!(t.check_restored(2), Ok(()));
        assert!(t.check_restored(1).is_err());
    }
}
