//! The declaration-extent table (spec/ast.md §19, WP-2090): the source
//! extents of every declaration as written, for tooling clients that
//! rewrite source (remove, move, rename or wrap a declaration).
//!
//! Opt-in, like the written-reference table (§18): off by default, and
//! off every recording site is one branch on [`DeclExtents::is_on`] and
//! nothing is allocated. On, the parser records one [`DeclExtent`] per
//! declaration it parsed, keyed by the declaration and, for a declaration
//! written more than once under one id (an explicit instantiation, a
//! repeated explicit specialization), its ordinal among those writings
//! ([`DeclExtents::rows_of`]; WP-2470); a template shares its pattern's
//! rows. Implicit declarations and instantiations have none.
//! Module and import declarations, which are preprocessing directives
//! with no declaration node, have rows of their own kind
//! ([`ModuleDeclExtent`], spec/ast.md §19.9).

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

/// What a [`ModuleDeclExtent`] is (spec/ast.md §19.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
#[repr(u8)]
pub enum ModuleDeclKind {
    /// A module-declaration ([module.unit]/1): `export module M:P;`.
    #[default]
    Module,
    /// A module-import-declaration of a named module or a partition
    /// ([module.import]/1): `import M;`, `import :P;`.
    Import,
    /// A module-import-declaration of a header unit: `import <h>;`,
    /// `import "h";`.
    HeaderImport,
}

impl ModuleDeclKind {
    /// The dump's spelling.
    pub const fn spelling(self) -> &'static str {
        match self {
            ModuleDeclKind::Module => "ModuleDecl",
            ModuleDeclKind::Import => "ImportDecl",
            ModuleDeclKind::HeaderImport => "HeaderImportDecl",
        }
    }
}

/// The extents of a module-declaration or a module-import-declaration
/// as written (spec/ast.md §19.9; 64 bytes). They are preprocessing
/// directives ([cpp.module], [cpp.import]) that declare no declaration
/// node, so the row is keyed by its position in the unit, not by a
/// [`DeclId`], and it names the parts a [`DeclExtent`] has no field for:
/// the keyword, the partition and the attributes. Every [`Span`] is a
/// token range as in a [`DeclExtent`]; an absent part is
/// [`Span::INVALID`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, kiln_snap::Snap)]
pub struct ModuleDeclExtent {
    /// The declaration: from `export` (or the keyword) to the `;`.
    pub full: Span,
    /// `export`.
    pub export: Span,
    /// `module` or `import`.
    pub keyword: Span,
    /// The module-name, every dotted component (`a.b`), or the
    /// header-name of a header unit (`<h>`, `"h"`); invalid for
    /// `import :P;`.
    pub name: Span,
    /// The module-partition, from its `:` to its last component.
    pub partition: Span,
    /// The attribute-specifier-seq before the `;`.
    pub attrs: Span,
    /// The `;`; invalid when it is missing (an error).
    pub semi: Span,
    /// The index of the first declaration made after it (the arena's
    /// next id when it was recorded): the dump prints it before that
    /// declaration's row.
    pub before: u32,
    /// What it is.
    pub kind: ModuleDeclKind,
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
    /// By declaration index: its first row + 1, or 0 for none. Grown on
    /// demand, so it ends at the last declaration with a row.
    index: Vec<u32>,
    /// The later rows of a declaration written more than once under one
    /// [`DeclId`] (a redeclaration the AST merges into the declaration it
    /// redeclares, an explicit instantiation of a specialization; WP-2470):
    /// `(first row, row)`, ascending.
    more: Vec<(u32, u32)>,
    /// The module and import declarations, in the order the parser saw
    /// them (WP-2091).
    modules: Vec<ModuleDeclExtent>,
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

    /// The number of rows (a template sharing its pattern's counts once;
    /// a declaration written more than once counts once per writing).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True when there are no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// `d`'s first row.
    fn first_row(&self, d: DeclId) -> Option<usize> {
        match self.index.get(d.index() as usize) {
            Some(&r) if r != 0 => Some(r as usize - 1),
            _ => None,
        }
    }

    /// The later rows of the declaration whose first row is `first`.
    fn more_of(&self, first: usize) -> &[(u32, u32)] {
        let f = first as u32;
        let lo = self.more.partition_point(|&(x, _)| x < f);
        let hi = self.more.partition_point(|&(x, _)| x <= f);
        &self.more[lo..hi]
    }

    /// `d`'s last row: that of the last declaration of it written.
    fn row_of(&self, d: DeclId) -> Option<usize> {
        let first = self.first_row(d)?;
        Some(self.more_of(first).last().map_or(first, |&(_, r)| r as usize))
    }

    /// The extents of declaration `d`, if it was parsed with recording
    /// on: the row of the last declaration of `d` written, when `d` was
    /// written more than once under one id ([`Self::rows_of`] has every
    /// one).
    pub fn get(&self, d: DeclId) -> Option<&DeclExtent> {
        self.row_of(d).map(|r| &self.rows[r])
    }

    /// Every row of declaration `d`, in source order: one per declaration
    /// of `d` written under this id (spec/ast.md §19.1; WP-2470).
    pub fn rows_of(&self, d: DeclId) -> impl Iterator<Item = &DeclExtent> + '_ {
        let first = self.first_row(d);
        let more = first.map_or(&[][..], |f| self.more_of(f));
        first.into_iter().chain(more.iter().map(|&(_, r)| r as usize)).map(|r| &self.rows[r])
    }

    /// Every row, by declaration in declaration-id order, a declaration's
    /// rows in source order ([`Self::rows_of`]).
    pub fn iter(&self) -> impl Iterator<Item = (DeclId, &DeclExtent)> + '_ {
        self.index.iter().enumerate().filter(|(_, r)| **r != 0).flat_map(|(i, _)| {
            let d = DeclId::new(u32::try_from(i).unwrap_or(u32::MAX));
            self.rows_of(d).map(move |e| (d, e))
        })
    }

    /// Records `e` as `d`'s extents (replacing its last row); nothing
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

    /// Starts a new, empty row for `d`, written by a later declaration of
    /// it under the same id (an explicit instantiation of a specialization
    /// declared before, a repeated explicit or partial specialization
    /// declaration, an opaque enumeration redeclared; WP-2470): the
    /// earlier rows stay, and the writes that follow (`set`, `entry`,
    /// `get_mut`) go to the new one. Nothing when off; the first row
    /// when `d` has none.
    pub fn redeclare(&mut self, d: DeclId) {
        if !self.on {
            return;
        }
        let Some(first) = self.first_row(d) else {
            self.set(d, DeclExtent::default());
            return;
        };
        self.rows.push(DeclExtent::default());
        let r = u32::try_from(self.rows.len() - 1).expect("extent table exhausted");
        // `r` is the largest row: it goes after `first`'s other rows.
        let f = first as u32;
        let at = self.more.partition_point(|&(x, _)| x <= f);
        self.more.insert(at, (f, r));
    }

    /// The module and import declarations' rows, in source order.
    pub fn module_decls(&self) -> &[ModuleDeclExtent] {
        &self.modules
    }

    /// Records a module or import declaration's row; nothing when off.
    pub fn push_module_decl(&mut self, e: ModuleDeclExtent) {
        if self.on {
            self.modules.push(e);
        }
    }

    /// Gives the table's spare capacity back (WP-2092): the parser calls
    /// it once the translation unit is parsed, when no more rows come.
    pub fn shrink_to_fit(&mut self) {
        self.rows.shrink_to_fit();
        self.index.shrink_to_fit();
        self.more.shrink_to_fit();
        self.modules.shrink_to_fit();
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

    /// True when `alias` shares `d`'s rows ([`Self::share`]).
    pub fn shares(&self, alias: DeclId, d: DeclId) -> bool {
        self.first_row(alias).is_some_and(|a| Some(a) == self.first_row(d))
    }

    /// Drops `d`'s row, for a declaration the parse made and then
    /// discarded (the `void` of `(void)`): the row itself when it is the
    /// last one, else only `d`'s entry.
    pub fn forget(&mut self, d: DeclId) {
        let Some(r) = self.row_of(d) else { return };
        if let Some(k) = self.more.iter().position(|&(_, x)| x as usize == r) {
            // A later row of `d`: only it goes.
            self.more.remove(k);
            if r + 1 == self.rows.len() {
                self.rows.pop();
            }
        } else if r + 1 == self.rows.len() {
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
        // A later row's first row is before it.
        self.more.retain(|&(_, r)| r < n);
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
        // A later row comes after its first row, inside the rows, and the
        // list ascends.
        let later_ok = self.more.iter().all(|&(f, r)| f < r && (r as usize) < self.rows.len())
            && self.more.windows(2).all(|w| w[0] < w[1]);
        if !later_ok {
            return Err("declaration extent later rows out of order");
        }
        if self.modules.iter().any(|m| m.before as usize > decls + 1) {
            return Err("module declaration extent past the decl arena");
        }
        Ok(())
    }

    /// The memory row (`decl_extents`), only when recording is on.
    pub fn mem_row(&self) -> Option<MemRow> {
        if !self.on {
            return None;
        }
        let sz = size_of::<DeclExtent>() as u64;
        let msz = size_of::<ModuleDeclExtent>() as u64;
        Some(MemRow::new(
            "decl_extents",
            (self.rows.len() + self.modules.len()) as u64,
            self.rows.len() as u64 * sz
                + self.index.len() as u64 * 4
                + self.more.len() as u64 * 8
                + self.modules.len() as u64 * msz,
            self.rows.capacity() as u64 * sz
                + self.index.capacity() as u64 * 4
                + self.more.capacity() as u64 * 8
                + self.modules.capacity() as u64 * msz,
        ))
    }

    /// The dump (spec/ast.md §19.7): one line per declaration with
    /// extents, in declaration-id order; a module or import declaration's
    /// line comes before the row of the first declaration made after it.
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
        let mut mods = self.modules.iter().peekable();
        let module_line = |out: &mut String, m: &ModuleDeclExtent| {
            let text = |s: Span| sm.span_text(s).unwrap_or("?");
            let spelled = match (m.name.lo.is_valid(), m.partition.lo.is_valid()) {
                (true, true) => Span::new(m.name.lo, m.partition.hi),
                (true, false) => m.name,
                (false, _) => m.partition,
            };
            let end = if m.semi.lo.is_valid() { ";" } else { "-" };
            let _ = write!(
                out,
                "{} '{}'  full={} {end}",
                m.kind.spelling(),
                if spelled.lo.is_valid() { text(spelled) } else { "" },
                range(m.full)
            );
            for (label, s) in [
                ("export", m.export),
                ("keyword", m.keyword),
                ("name", m.name),
                ("partition", m.partition),
                ("attrs", m.attrs),
            ] {
                if s.lo.is_valid() {
                    let _ = write!(out, " {label}={} '{}'", range(s), text(s));
                }
            }
            out.push('\n');
        };
        for (d, e) in self.iter() {
            while let Some(m) = mods.next_if(|m| m.before <= d.index()) {
                module_line(&mut out, m);
            }
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
        for m in mods {
            module_line(&mut out, m);
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
        assert_eq!(size_of::<ModuleDeclExtent>(), 64);
    }

    #[test]
    fn module_rows_only_when_on_and_shrink() {
        let mut t = DeclExtents::default();
        t.push_module_decl(ModuleDeclExtent::default());
        assert!(t.module_decls().is_empty());
        t.set_on(true);
        for i in 1..=5 {
            t.set(row_id(i), row(i, i + 1));
        }
        t.push_module_decl(ModuleDeclExtent { before: 9, ..ModuleDeclExtent::default() });
        assert_eq!(t.module_decls().len(), 1);
        assert!(t.rows.capacity() > t.rows.len());
        t.shrink_to_fit();
        assert_eq!(t.rows.capacity(), t.rows.len());
        assert_eq!(t.index.capacity(), t.index.len());
        let m = t.mem_row().map(|r| (r.live_bytes, r.reserved_bytes));
        assert!(m.is_some_and(|(b, r)| b == r), "{m:?}");
        // A module row's position must be inside the arena.
        assert_eq!(t.check_restored(8), Ok(()));
        assert!(t.check_restored(7).is_err());
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
    fn redeclare_keeps_every_row() {
        let mut t = DeclExtents::default();
        t.redeclare(row_id(3));
        assert!(t.is_empty());
        t.set_on(true);
        let (a, b, alias) = (row_id(3), row_id(5), row_id(6));
        // No row yet: the first one.
        t.redeclare(a);
        t.set(a, row(1, 2));
        t.set(b, row(3, 4));
        t.share(alias, b);
        t.redeclare(a);
        t.set(a, row(5, 6));
        t.redeclare(b);
        t.set(b, row(7, 8));
        t.redeclare(a);
        t.set(a, row(9, 10));
        let his = |t: &DeclExtents, d| t.rows_of(d).map(|e| e.full.hi.to_raw()).collect::<Vec<_>>();
        assert_eq!(his(&t, a), [2, 6, 10]);
        assert_eq!(his(&t, b), [4, 8]);
        // A template sharing its pattern's row shares its later rows.
        assert_eq!(his(&t, alias), [4, 8]);
        assert_eq!(t.get(a).map(|e| e.full.hi.to_raw()), Some(10));
        assert_eq!(t.len(), 5);
        let all: Vec<_> = t.iter().map(|(d, e)| (d.index(), e.full.hi.to_raw())).collect();
        assert_eq!(all, [(3, 2), (3, 6), (3, 10), (5, 4), (5, 8), (6, 4), (6, 8)]);
        assert_eq!(t.check_restored(6), Ok(()));
        // The last declaration made and dropped takes only its row.
        t.forget(a);
        assert_eq!(his(&t, a), [2, 6]);
        let m = t.mark();
        t.redeclare(b);
        t.set(b, row(11, 12));
        t.truncate(m);
        assert_eq!(his(&t, b), [4, 8]);
        t.shrink_to_fit();
        let m = t.mem_row().map(|r| (r.live_bytes, r.reserved_bytes));
        assert!(m.is_some_and(|(b, r)| b == r), "{m:?}");
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
