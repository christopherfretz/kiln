//! The written-reference table (spec/ast.md §18, WP-1930): an opt-in side
//! table of the names written in the source, each with the declaration
//! it denotes, for tooling clients.
//!
//! Off by default; when off every recording site is one branch on
//! [`WrittenRefs::is_on`] and nothing is allocated. Entries are appended
//! by the parser (through sema's `note_written_*` helpers) in parse
//! order; the overload-resolution funnels patch the entity of an
//! overloaded name once, and declarations narrow `enclosing` by claims
//! (§18.3).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::num::NonZeroU32;

use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::ids::{DeclId, ExprId};
use crate::mem::MemRow;
use crate::types::TypeNames;

/// The id of a table entry (index + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WrittenRefId(NonZeroU32);

impl WrittenRefId {
    /// The entry's index in [`WrittenRefs::iter`] order.
    #[inline]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    fn from_index(i: usize) -> Self {
        let n = u32::try_from(i + 1).expect("written-reference table exhausted");
        WrittenRefId(NonZeroU32::new(n).expect("index + 1 is never 0"))
    }
}

/// The syntactic role of a written name (spec/ast.md §18.4). The full
/// set; stage 1 fills `Expr`, `Member` and `Nns`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RefRole {
    /// An id-expression, enumerator, `&f`, `case E:`, lambda capture.
    Expr,
    /// A class member access name: `a.m`, `p->m()`, `p->~T()`.
    Member,
    /// A type name in a type-specifier context (stage 2).
    Type,
    /// A template name (stage 2).
    Template,
    /// A nested-name-specifier component.
    Nns,
    /// A base specifier (stage 2).
    Base,
    /// A member-initializer id (stage 2).
    MemInit,
    /// The name of a using-declaration (stage 3).
    UsingDecl,
    /// The namespace of a using-directive (stage 3).
    UsingDirective,
    /// The target of a namespace alias (stage 3).
    NamespaceAlias,
    /// An elaborated or friend reference to an existing entity (stage 3).
    Elaborated,
    /// The target of an explicit specialization or instantiation (stage 3).
    SpecTarget,
    /// A concept name (stage 3).
    Concept,
    /// A designator (stage 3).
    Designator,
}

impl RefRole {
    /// The role as the dump spells it.
    pub const fn spelling(self) -> &'static str {
        match self {
            RefRole::Expr => "expr",
            RefRole::Member => "member",
            RefRole::Type => "type",
            RefRole::Template => "template",
            RefRole::Nns => "nns",
            RefRole::Base => "base",
            RefRole::MemInit => "mem-init",
            RefRole::UsingDecl => "using-decl",
            RefRole::UsingDirective => "using-directive",
            RefRole::NamespaceAlias => "ns-alias",
            RefRole::Elaborated => "elaborated",
            RefRole::SpecTarget => "spec-target",
            RefRole::Concept => "concept",
            RefRole::Designator => "designator",
        }
    }
}

/// The `qualifier` link of an entry: none, the global scope (a leading
/// `::`), or the entry of the component to the name's left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RefQualifier(u32);

/// [`RefQualifier`] decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualLink {
    /// An unqualified name.
    None,
    /// A name after a leading `::`.
    Global,
    /// A name after the component recorded as this entry.
    Entry(WrittenRefId),
}

impl RefQualifier {
    /// An unqualified name.
    pub const NONE: RefQualifier = RefQualifier(0);
    /// A name after a leading `::`.
    pub const GLOBAL: RefQualifier = RefQualifier(u32::MAX);

    /// A name after the component recorded as `id`.
    pub const fn entry(id: WrittenRefId) -> RefQualifier {
        RefQualifier(id.0.get())
    }

    /// The link decoded.
    pub fn link(self) -> QualLink {
        match self.0 {
            0 => QualLink::None,
            u32::MAX => QualLink::Global,
            n => QualLink::Entry(WrittenRefId(NonZeroU32::new(n).expect("n is not 0"))),
        }
    }
}

/// [`WrittenRef::flags`]: a dependent name in a template pattern.
pub const REF_DEPENDENT: u8 = 1;
/// [`WrittenRef::flags`]: lookup or overload resolution failed.
pub const REF_UNRESOLVED: u8 = 2;

/// One written name (spec/ast.md §18.2; 28 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrittenRef {
    /// The name as written: start of its first token to the end of its
    /// last token, in kiln token locations.
    pub range: Span,
    /// The declaration the name denotes after resolution.
    pub entity: Option<DeclId>,
    /// What lookup found, when it differs from `entity`.
    pub found: Option<DeclId>,
    /// The innermost declaration containing the name (§18.3).
    pub enclosing: Option<DeclId>,
    /// The component to the name's left.
    pub qualifier: RefQualifier,
    /// The syntactic role.
    pub role: RefRole,
    /// [`REF_DEPENDENT`], [`REF_UNRESOLVED`].
    pub flags: u8,
}

/// A position in the table and the decl arena, for claims and rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefMark {
    /// Entries before the mark.
    pub entries: u32,
    /// The number of declarations when the mark was taken: a decl with a
    /// larger index was created after it.
    pub decls: u32,
}

/// The table (spec/ast.md §18). Transient in snapshots (§17).
#[derive(Clone, Debug, Default)]
pub struct WrittenRefs {
    on: bool,
    entries: Vec<WrittenRef>,
    /// Entries of overloaded names awaiting overload resolution, by the
    /// overload set's expression.
    pending: HashMap<ExprId, WrittenRefId>,
}

impl WrittenRefs {
    /// True when names are recorded.
    #[inline]
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns recording on or off (entries already made stay).
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry `id`.
    pub fn get(&self, id: WrittenRefId) -> &WrittenRef {
        &self.entries[id.index()]
    }

    /// Every entry with its id, in recording order.
    pub fn iter(&self) -> impl Iterator<Item = (WrittenRefId, &WrittenRef)> + '_ {
        self.entries.iter().enumerate().map(|(i, r)| (WrittenRefId::from_index(i), r))
    }

    /// The entries whose `enclosing` is `d`.
    pub fn enclosed_by(&self, d: DeclId) -> impl Iterator<Item = (WrittenRefId, &WrittenRef)> + '_ {
        self.iter().filter(move |(_, r)| r.enclosing == Some(d))
    }

    /// Appends `r`; `None` (and nothing recorded) when off.
    pub fn push(&mut self, r: WrittenRef) -> Option<WrittenRefId> {
        if !self.on {
            return None;
        }
        self.entries.push(r);
        Some(WrittenRefId::from_index(self.entries.len() - 1))
    }

    /// Remembers that entry `id` names the overload set `e`, for
    /// [`resolve`](Self::resolve).
    pub fn defer(&mut self, id: WrittenRefId, e: ExprId) {
        self.pending.insert(e, id);
    }

    /// True when an overloaded name awaits resolution.
    #[inline]
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The overload set `from` is resolved as the set `to` (an implicit
    /// member call of a template-id): the entry naming `from` follows.
    pub fn retarget(&mut self, from: ExprId, to: ExprId) {
        if let Some(id) = self.pending.remove(&from) {
            self.pending.insert(to, id);
        }
    }

    /// Overload resolution selected `entity` (found as `found`) for the
    /// overload set `e`: patches the entry naming `e`, once.
    pub fn resolve(&mut self, e: ExprId, entity: DeclId, found: Option<DeclId>) {
        if let Some(id) = self.pending.remove(&e) {
            let r = &mut self.entries[id.index()];
            r.entity = Some(entity);
            r.found = found.filter(|&f| f != entity);
            r.flags &= !(REF_DEPENDENT | REF_UNRESOLVED);
        }
    }

    /// The table position, with `decls` the decl arena's length now.
    pub fn mark(&self, decls: usize) -> RefMark {
        RefMark { entries: self.entries.len() as u32, decls: decls as u32 }
    }

    /// Drops the entries recorded after `m` (and their pending patches).
    pub fn truncate(&mut self, m: RefMark) {
        let n = m.entries as usize;
        if n >= self.entries.len() {
            return;
        }
        self.entries.truncate(n);
        self.pending.retain(|_, id| id.index() < n);
    }

    /// Declaration `d`, whose names were parsed after `m`, claims them:
    /// every entry after `m` whose `enclosing` existed at `m` gets
    /// `enclosing = d` (an entry of a declaration nested inside keeps
    /// its own; spec/ast.md §18.3).
    pub fn claim(&mut self, m: RefMark, d: DeclId) {
        let start = m.entries as usize;
        for r in self.entries.iter_mut().skip(start) {
            if r.enclosing.is_none_or(|e| e.index() <= m.decls) {
                r.enclosing = Some(d);
            }
        }
    }

    /// The memory row (`written_refs`), only when recording is on.
    pub fn mem_row(&self) -> Option<MemRow> {
        if !self.on {
            return None;
        }
        let sz = size_of::<WrittenRef>() as u64;
        let pend = (size_of::<(ExprId, WrittenRefId)>() as u64) * 2;
        Some(MemRow::new(
            "written_refs",
            self.entries.len() as u64,
            self.entries.len() as u64 * sz + self.pending.len() as u64 * pend,
            self.entries.capacity() as u64 * sz + self.pending.capacity() as u64 * pend,
        ))
    }

    /// The dump (spec/ast.md §18.6): one line per entry.
    pub fn dump(&self, ast: &crate::Ast, sm: &SourceManager, main: Option<FileId>) -> String {
        let names = crate::diag_names::DiagNames::new(ast, Some(sm), 22).closures_as_lambda();
        // (file prefix outside the main file, `line:col`) of a spelling.
        let pos2 = |loc: Loc| -> (String, String) {
            match sm.line_col(sm.spelling_loc(loc)) {
                Some(lc) if Some(lc.file) == main => {
                    (String::new(), format!("{}:{}", lc.line, lc.col))
                }
                Some(lc) => {
                    let n = sm.file_name(lc.file);
                    let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
                    (format!("{base}:"), format!("{}:{}", lc.line, lc.col))
                }
                None => (String::new(), "?".to_string()),
            }
        };
        let pos = |loc: Loc| -> String {
            let (f, lc) = pos2(loc);
            f + &lc
        };
        let decl = |d: DeclId, out: &mut String| {
            let mut s = String::new();
            // A parameter prints unqualified, as a local variable does.
            let scoped = ast.decls.get(d).kind() != crate::decl::DeclKind::ParmVar;
            names.write_decl_name(d, scoped, &mut s);
            if s.is_empty() {
                s.push_str("(anonymous)");
            }
            out.push_str(&s);
            out.push('@');
            out.push_str(&pos(ast.decls.get(d).loc()));
        };
        let mut out = String::new();
        for r in &self.entries {
            let (lf, lo) = pos2(r.range.lo);
            let (hf, hi) = pos2(r.range.hi);
            let hi = if hf == lf { hi } else { hf + &hi };
            let lo = lf + &lo;
            let text = sm.span_text(r.range).unwrap_or("?");
            let _ = write!(out, "{lo}-{hi}  {}  '{text}' -> ", r.role.spelling());
            match r.entity {
                Some(d) => decl(d, &mut out),
                None if r.flags & REF_DEPENDENT != 0 => out.push_str("(dependent)"),
                None => out.push_str("(unresolved)"),
            }
            if let Some(f) = r.found {
                out.push_str(" found=");
                decl(f, &mut out);
            }
            match r.qualifier.link() {
                QualLink::None => {}
                QualLink::Global => out.push_str(" q=::"),
                QualLink::Entry(q) => {
                    out.push_str(" q=");
                    out.push_str(&pos(self.get(q).range.lo));
                }
            }
            match r.enclosing {
                Some(d) if d == ast.tu => out.push_str(" in=(tu)"),
                Some(d) => {
                    out.push_str(" in=");
                    decl(d, &mut out);
                }
                None => {}
            }
            if r.entity.is_some() && r.flags & REF_DEPENDENT != 0 {
                out.push_str(" dep");
            }
            if sm.is_macro_loc(r.range.lo) {
                out.push_str(" exp=");
                out.push_str(&pos(sm.expansion_loc(r.range.lo)));
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(enclosing: Option<DeclId>) -> WrittenRef {
        WrittenRef {
            range: Span::new(Loc::from_raw(1), Loc::from_raw(2)),
            entity: None,
            found: None,
            enclosing,
            qualifier: RefQualifier::NONE,
            role: RefRole::Expr,
            flags: 0,
        }
    }

    #[test]
    fn entry_is_28_bytes() {
        assert_eq!(size_of::<WrittenRef>(), 28);
        assert_eq!(size_of::<Option<WrittenRefId>>(), 4);
    }

    #[test]
    fn off_records_nothing() {
        let mut t = WrittenRefs::default();
        assert!(!t.is_on());
        assert_eq!(t.push(entry(None)), None);
        assert!(t.is_empty());
        assert!(t.mem_row().is_none());
        assert_eq!(t.entries.capacity(), 0);
    }

    #[test]
    fn qualifier_links_round_trip() {
        let id = WrittenRefId::from_index(4);
        assert_eq!(RefQualifier::entry(id).link(), QualLink::Entry(id));
        assert_eq!(RefQualifier::GLOBAL.link(), QualLink::Global);
        assert_eq!(RefQualifier::NONE.link(), QualLink::None);
    }

    #[test]
    fn innermost_claim_wins() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let ns = DeclId::new(1);
        let outer_mark = t.mark(1);
        t.push(entry(Some(ns)));
        let inner_mark = t.mark(1);
        t.push(entry(Some(ns)));
        let inner = DeclId::new(2);
        t.claim(inner_mark, inner);
        let outer = DeclId::new(3);
        t.claim(outer_mark, outer);
        let encl: Vec<_> = t.iter().map(|(_, r)| r.enclosing).collect();
        assert_eq!(encl, [Some(outer), Some(inner)]);
        // A claim is idempotent.
        t.claim(outer_mark, outer);
        assert_eq!(t.get(WrittenRefId::from_index(1)).enclosing, Some(inner));
    }

    #[test]
    fn resolve_patches_once_and_truncate_drops_pending() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let id = t.push(entry(None)).unwrap();
        let e = ExprId::new(7);
        t.defer(id, e);
        let f = DeclId::new(5);
        t.resolve(e, f, Some(f));
        assert_eq!(t.get(id).entity, Some(f));
        assert_eq!(t.get(id).found, None);
        t.resolve(e, DeclId::new(6), None);
        assert_eq!(t.get(id).entity, Some(f));
        let m = t.mark(0);
        let id2 = t.push(entry(None)).unwrap();
        t.defer(id2, ExprId::new(8));
        t.truncate(m);
        assert_eq!(t.len(), 1);
        assert!(!t.has_pending());
    }
}
