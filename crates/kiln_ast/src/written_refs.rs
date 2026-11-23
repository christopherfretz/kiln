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
/// set; stage 1 fills `Expr`, `Member` and `Nns`, stage 2 `Type`,
/// `Template`, `Base` and `MemInit`, stage 3 the declarative roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RefRole {
    /// An id-expression, enumerator, `&f`, `case E:`, lambda capture.
    Expr,
    /// A class member access name: `a.m`, `p->m()`, `p->~T()`.
    Member,
    /// A type name in a type-specifier context (stage 2).
    Type,
    /// The template name of a template-id in a type context, a class
    /// template named for argument deduction, a template template
    /// argument (stage 2).
    Template,
    /// A nested-name-specifier component.
    Nns,
    /// The class name of a base-specifier (stage 2).
    Base,
    /// A mem-initializer-id, naming a base, a member or (delegating) the
    /// class itself (stage 2).
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
    /// The declarator-id or class-head-name of a declaration that
    /// redeclares an entity declared before it, written qualified
    /// (`void A::f() {}`, `int C::x = 0;`, `struct C::In {}`; stage 3).
    Redecl,
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
            RefRole::Redecl => "redecl",
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
/// [`WrittenRef::flags`]: a member or member template name written after
/// `.` or `->` (spec/ast.md §18.7.2).
pub const REF_MEMBER: u8 = 4;

/// How lookup found a written name (spec/ast.md §18.7.2, WP-2130).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum RefLookup {
    /// No entity: a dependent name or a failed resolution.
    #[default]
    None,
    /// The scope walk ([basic.lookup.unqual]).
    Unqualified,
    /// Lookup in the scope a nested-name-specifier names.
    Qualified,
    /// Class member access (`.`, `->`).
    Member,
    /// The selected function was found only by argument-dependent lookup.
    Adl,
    /// Ordinary and argument-dependent lookup both found it.
    Both,
    /// An implicit builtin function declaration.
    Builtin,
    /// An injected-class-name ([class.pre]/2, [temp.local]/1).
    Injected,
}

impl RefLookup {
    /// The kind as the dump spells it.
    pub const fn spelling(self) -> &'static str {
        match self {
            RefLookup::None => "none",
            RefLookup::Unqualified => "unqualified",
            RefLookup::Qualified => "qualified",
            RefLookup::Member => "member",
            RefLookup::Adl => "adl",
            RefLookup::Both => "both",
            RefLookup::Builtin => "builtin",
            RefLookup::Injected => "injected",
        }
    }
}

/// The id of an interned argument-dependent set (index + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AdlSetId(NonZeroU32);

/// The associated namespaces, classes and enumerations of an
/// argument-dependent lookup ([basic.lookup.argdep]/3) in discovery order
/// (spec/ast.md §18.7.4), interned as one list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AdlSet {
    list: Box<[DeclId]>,
    namespaces: u32,
    classes: u32,
}

impl AdlSet {
    /// The set of `namespaces` (primary contexts), `classes` and `enums`
    /// (first declarations).
    pub fn new(namespaces: &[DeclId], classes: &[DeclId], enums: &[DeclId]) -> AdlSet {
        let list: Vec<DeclId> = namespaces.iter().chain(classes).chain(enums).copied().collect();
        AdlSet {
            list: list.into_boxed_slice(),
            namespaces: namespaces.len() as u32,
            classes: classes.len() as u32,
        }
    }

    /// The associated namespaces (the translation unit for the global
    /// namespace).
    pub fn namespaces(&self) -> &[DeclId] {
        &self.list[..self.namespaces as usize]
    }

    /// The associated classes.
    pub fn classes(&self) -> &[DeclId] {
        let n = self.namespaces as usize;
        &self.list[n..n + self.classes as usize]
    }

    /// The associated enumerations.
    pub fn enums(&self) -> &[DeclId] {
        &self.list[(self.namespaces + self.classes) as usize..]
    }

    /// True when nothing is associated.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// The side row of an entry with more provenance than its `how`
/// (spec/ast.md §18.7.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RefExtra {
    udir: Option<DeclId>,
    adl: Option<AdlSetId>,
}

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
    /// [`REF_DEPENDENT`], [`REF_UNRESOLVED`], [`REF_MEMBER`].
    pub flags: u8,
    /// How lookup found the name (§18.7.2).
    pub how: RefLookup,
}

impl WrittenRef {
    /// The `how` the entry's other fields imply (spec/ast.md §18.7.5):
    /// `None` without an entity, `Member` after `.`/`->`, `Qualified`
    /// after a qualifier, else `Unqualified`.
    pub fn default_how(&self) -> RefLookup {
        if self.entity.is_none() {
            RefLookup::None
        } else if self.flags & REF_MEMBER != 0 {
            RefLookup::Member
        } else if self.qualifier != RefQualifier::NONE {
            RefLookup::Qualified
        } else {
            RefLookup::Unqualified
        }
    }
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
    /// The side rows (§18.7.1): the using-directive and the
    /// argument-dependent set of the entries that have one.
    extra: HashMap<WrittenRefId, RefExtra>,
    /// The interned argument-dependent sets, by `AdlSetId` index.
    adl_sets: Vec<AdlSet>,
    adl_index: HashMap<AdlSet, AdlSetId>,
    /// The using-directive in namespace `.0` that nominates `.1`, once
    /// looked up (§18.7.3).
    udir_cache: HashMap<(DeclId, DeclId), Option<DeclId>>,
    /// The entries of field designators, by the name's location: sema
    /// patches them when it finds the member (§18.4, stage 3).
    designators: HashMap<Loc, WrittenRefId>,
    /// What the explicit instantiation whose declarator-id is at the
    /// location names (a template or a member of a class template
    /// specialization): sema finds it, the parser records the name
    /// after (§18.4, stage 3).
    inst_target: Option<(Loc, DeclId)>,
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

    /// Changes the role of entry `id` (a type name that turned out to be
    /// a base-specifier's or mem-initializer's class name, §18.4).
    pub fn set_role(&mut self, id: WrittenRefId, role: RefRole) {
        self.entries[id.index()].role = role;
    }

    /// Sets what entry `id` names (a name recorded before it was
    /// resolved: a mem-initializer-id).
    pub fn set_target(
        &mut self,
        id: WrittenRefId,
        entity: Option<DeclId>,
        found: Option<DeclId>,
        flags: u8,
    ) {
        let r = &mut self.entries[id.index()];
        r.entity = entity;
        r.found = found;
        r.flags = flags;
    }

    /// Overload resolution selected `entity` (found as `found`) for the
    /// overload set `e`: patches the entry naming `e`, once, and returns
    /// it.
    pub fn resolve(
        &mut self,
        e: ExprId,
        entity: DeclId,
        found: Option<DeclId>,
    ) -> Option<WrittenRefId> {
        let id = self.pending.remove(&e)?;
        let r = &mut self.entries[id.index()];
        r.entity = Some(entity);
        r.found = found.filter(|&f| f != entity);
        r.flags &= !(REF_DEPENDENT | REF_UNRESOLVED);
        Some(id)
    }

    /// Remembers that entry `id` is the field designator at `loc`, for
    /// [`designator_at`](Self::designator_at).
    pub fn defer_designator(&mut self, loc: Loc, id: WrittenRefId) {
        self.designators.insert(loc, id);
    }

    /// The entry of the field designator whose name is at `loc` (one
    /// branch when there is none).
    #[inline]
    pub fn designator_at(&self, loc: Loc) -> Option<WrittenRefId> {
        if self.designators.is_empty() { None } else { self.designators.get(&loc).copied() }
    }

    /// Remembers that the explicit instantiation whose declarator-id is
    /// at `loc` names `target`, for [`take_inst_target`](Self::take_inst_target).
    pub fn set_inst_target(&mut self, loc: Loc, target: DeclId) {
        self.inst_target = Some((loc, target));
    }

    /// What the explicit instantiation whose declarator-id is at `loc`
    /// names, once (`None` when sema matched nothing there).
    pub fn take_inst_target(&mut self, loc: Loc) -> Option<DeclId> {
        match self.inst_target.take() {
            Some((l, t)) if l == loc => Some(t),
            _ => None,
        }
    }

    /// The entry naming the overload set `e`, while it awaits resolution.
    pub fn pending_for(&self, e: ExprId) -> Option<WrittenRefId> {
        self.pending.get(&e).copied()
    }

    /// Sets how entry `id` was found (§18.7.2).
    pub fn set_how(&mut self, id: WrittenRefId, how: RefLookup) {
        self.entries[id.index()].how = how;
    }

    /// Sets (or clears) the using-directive entry `id` was found through
    /// (§18.7.3).
    pub fn set_using_directive(&mut self, id: WrittenRefId, udir: Option<DeclId>) {
        match udir {
            Some(d) => self.extra.entry(id).or_default().udir = Some(d),
            None => {
                if let Some(x) = self.extra.get_mut(&id) {
                    x.udir = None;
                    if x.adl.is_none() {
                        self.extra.remove(&id);
                    }
                }
            }
        }
    }

    /// Sets the argument-dependent set of entry `id` (§18.7.4), interned.
    pub fn set_adl(&mut self, id: WrittenRefId, set: AdlSet) {
        let sid = if let Some(&s) = self.adl_index.get(&set) {
            s
        } else {
            let n = u32::try_from(self.adl_sets.len() + 1).expect("ADL sets exhausted");
            let s = AdlSetId(NonZeroU32::new(n).expect("index + 1 is never 0"));
            self.adl_sets.push(set.clone());
            self.adl_index.insert(set, s);
            s
        };
        self.extra.entry(id).or_default().adl = Some(sid);
    }

    /// The using-directive entry `id` was found through (§18.7.3).
    pub fn using_directive(&self, id: WrittenRefId) -> Option<DeclId> {
        self.extra.get(&id).and_then(|x| x.udir)
    }

    /// The interned id of entry `id`'s argument-dependent set: equal ids
    /// are equal sets.
    pub fn adl_set_id(&self, id: WrittenRefId) -> Option<AdlSetId> {
        self.extra.get(&id).and_then(|x| x.adl)
    }

    /// Entry `id`'s argument-dependent set (§18.7.4).
    pub fn adl_set(&self, id: WrittenRefId) -> Option<&AdlSet> {
        self.adl_set_id(id).map(|s| &self.adl_sets[(s.0.get() - 1) as usize])
    }

    /// The number of distinct argument-dependent sets.
    pub fn adl_set_count(&self) -> usize {
        self.adl_sets.len()
    }

    /// The cached using-directive of namespace `holder` nominating
    /// `nominated` (§18.7.3): `None` when not looked up yet.
    pub fn cached_directive(&self, holder: DeclId, nominated: DeclId) -> Option<Option<DeclId>> {
        self.udir_cache.get(&(holder, nominated)).copied()
    }

    /// Caches the answer of [`cached_directive`](Self::cached_directive).
    pub fn cache_directive(&mut self, holder: DeclId, nominated: DeclId, d: Option<DeclId>) {
        self.udir_cache.insert((holder, nominated), d);
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
        self.extra.retain(|id, _| id.index() < n);
        self.designators.retain(|_, id| id.index() < n);
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
        let desig = (size_of::<(Loc, WrittenRefId)>() as u64) * 2;
        let (prov_live, prov_reserved) = self.provenance_mem();
        Some(MemRow::new(
            "written_refs",
            self.entries.len() as u64,
            self.entries.len() as u64 * sz
                + self.pending.len() as u64 * pend
                + self.designators.len() as u64 * desig
                + prov_live,
            self.entries.capacity() as u64 * sz
                + self.pending.capacity() as u64 * pend
                + self.designators.capacity() as u64 * desig
                + prov_reserved,
        ))
    }

    /// The bytes (live, reserved) of the provenance side tables
    /// (§18.7.6): side rows, interned sets (each list kept in the set
    /// vector and as the interning key), the directive cache. A hash map
    /// slot counts twice its pair (control bytes and load factor).
    pub fn provenance_mem(&self) -> (u64, u64) {
        let ext = (size_of::<(WrittenRefId, RefExtra)>() as u64) * 2;
        let cache = (size_of::<((DeclId, DeclId), Option<DeclId>)>() as u64) * 2;
        let set = size_of::<AdlSet>() as u64;
        let key = (size_of::<(AdlSet, AdlSetId)>() as u64) * 2;
        let lists: u64 = self.adl_sets.iter().map(|s| 2 * 4 * s.list.len() as u64).sum();
        let live = self.extra.len() as u64 * ext
            + self.udir_cache.len() as u64 * cache
            + self.adl_sets.len() as u64 * (set + key)
            + lists;
        let reserved = self.extra.capacity() as u64 * ext
            + self.udir_cache.capacity() as u64 * cache
            + self.adl_sets.capacity() as u64 * set
            + self.adl_index.capacity() as u64 * key
            + lists;
        (live, reserved)
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
            // A function or template parameter prints unqualified, as a
            // local variable does.
            let scoped = !matches!(
                ast.decls.get(d).kind(),
                crate::decl::DeclKind::ParmVar
                    | crate::decl::DeclKind::TemplateTypeParm
                    | crate::decl::DeclKind::NonTypeTemplateParm
                    | crate::decl::DeclKind::TemplateTemplateParm
            );
            names.write_decl_name(d, scoped, &mut s);
            if s.is_empty() {
                s.push_str("(anonymous)");
            }
            out.push_str(&s);
            out.push('@');
            out.push_str(&pos(ast.decls.get(d).loc()));
        };
        let mut out = String::new();
        for (i, r) in self.entries.iter().enumerate() {
            let id = WrittenRefId::from_index(i);
            let (lf, lo) = pos2(r.range.lo);
            let (hf, hi) = pos2(r.range.hi);
            let hi = if hf == lf { hi } else { hf + &hi };
            let lo = lf + &lo;
            let text = sm.span_text(r.range).unwrap_or("?");
            if std::env::var_os("KILN_WREF_RAW").is_some() {
                let _ = write!(out, "#{} ", r.range.lo.to_raw());
            }
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
            // The lookup provenance (§18.7.5).
            self.dump_provenance(id, ast, &names, &pos, &decl, &mut out);
            out.push('\n');
        }
        out
    }

    /// The lookup-provenance fields of entry `id`'s dump line (§18.7.5).
    fn dump_provenance(
        &self,
        id: WrittenRefId,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        decl: &dyn Fn(DeclId, &mut String),
        out: &mut String,
    ) {
        let r = self.get(id);
        if r.how != r.default_how() {
            out.push_str(" how=");
            out.push_str(r.how.spelling());
        }
        if let Some(u) = self.using_directive(id) {
            out.push_str(" udir=");
            let mut s = String::new();
            if let Some(n) = ast.using_target(u) {
                names.write_decl_name(n, true, &mut s);
            }
            out.push_str(&s);
            out.push('@');
            out.push_str(&pos(ast.decls.get(u).loc()));
        }
        if let Some(set) = self.adl_set(id) {
            let list = |label: &str, v: &[DeclId], always: bool, out: &mut String| {
                if v.is_empty() && !always {
                    return;
                }
                out.push(' ');
                out.push_str(label);
                for (k, &d) in v.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    if d == ast.tu {
                        out.push_str("::");
                    } else if ast.decls.get(d).name() == crate::names::DeclName::EMPTY {
                        // An unnamed class or enumeration: its location
                        // names it, without a path-bearing spelling.
                        out.push_str("(anonymous)@");
                        out.push_str(&pos(ast.decls.get(d).loc()));
                    } else {
                        decl(d, out);
                    }
                }
            };
            list("adl-ns=", set.namespaces(), set.is_empty(), out);
            list("adl-cls=", set.classes(), false, out);
            list("adl-enum=", set.enums(), false, out);
        }
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
            how: RefLookup::None,
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
        assert_eq!(t.pending_for(e), Some(id));
        assert_eq!(t.resolve(e, f, Some(f)), Some(id));
        assert_eq!(t.pending_for(e), None);
        assert_eq!(t.get(id).entity, Some(f));
        assert_eq!(t.get(id).found, None);
        assert_eq!(t.resolve(e, DeclId::new(6), None), None);
        assert_eq!(t.get(id).entity, Some(f));
        let m = t.mark(0);
        let id2 = t.push(entry(None)).unwrap();
        t.defer(id2, ExprId::new(8));
        t.truncate(m);
        assert_eq!(t.len(), 1);
        assert!(!t.has_pending());
    }

    #[test]
    fn designators_and_inst_targets_are_found_by_location() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let (l1, l2) = (Loc::from_raw(10), Loc::from_raw(20));
        assert_eq!(t.designator_at(l1), None);
        let id = t.push(entry(None)).unwrap();
        t.defer_designator(l1, id);
        assert_eq!(t.designator_at(l1), Some(id));
        assert_eq!(t.designator_at(l2), None);
        let m = t.mark(0);
        let id2 = t.push(entry(None)).unwrap();
        t.defer_designator(l2, id2);
        t.truncate(m);
        assert_eq!(t.designator_at(l2), None, "truncate drops a dropped entry's designator");
        assert_eq!(t.designator_at(l1), Some(id));
        let f = DeclId::new(4);
        t.set_inst_target(l1, f);
        assert_eq!(t.take_inst_target(l2), None, "another location takes nothing");
        t.set_inst_target(l1, f);
        assert_eq!(t.take_inst_target(l1), Some(f));
        assert_eq!(t.take_inst_target(l1), None, "taken once");
    }

    #[test]
    fn default_how_follows_the_syntax() {
        let mut r = entry(None);
        assert_eq!(r.default_how(), RefLookup::None);
        r.entity = Some(DeclId::new(3));
        assert_eq!(r.default_how(), RefLookup::Unqualified);
        r.qualifier = RefQualifier::GLOBAL;
        assert_eq!(r.default_how(), RefLookup::Qualified);
        r.flags |= REF_MEMBER;
        assert_eq!(r.default_how(), RefLookup::Member);
    }

    #[test]
    fn adl_sets_are_interned_and_truncate_drops_side_rows() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let (ns, cls, en) = (DeclId::new(2), DeclId::new(3), DeclId::new(4));
        let first = t.push(entry(None)).unwrap();
        let second = t.push(entry(None)).unwrap();
        t.set_adl(first, AdlSet::new(&[ns], &[cls], &[en]));
        t.set_adl(second, AdlSet::new(&[ns], &[cls], &[en]));
        assert_eq!(t.adl_set_count(), 1);
        assert_eq!(t.adl_set_id(first), t.adl_set_id(second));
        let set = t.adl_set(first).unwrap();
        assert_eq!(
            (set.namespaces(), set.classes(), set.enums()),
            (&[ns][..], &[cls][..], &[en][..])
        );
        // A set differing only in where the lists split is another set.
        t.set_adl(second, AdlSet::new(&[ns, cls], &[], &[en]));
        assert_eq!(t.adl_set_count(), 2);
        assert_ne!(t.adl_set_id(first), t.adl_set_id(second));
        t.set_using_directive(second, Some(DeclId::new(9)));
        assert_eq!(t.using_directive(second), Some(DeclId::new(9)));
        assert_eq!(t.using_directive(first), None);
        t.set_how(second, RefLookup::Adl);
        assert_eq!(t.get(second).how, RefLookup::Adl);
        t.truncate(RefMark { entries: 1, decls: 0 });
        assert_eq!(t.using_directive(second), None);
        assert!(t.adl_set(first).is_some());
        assert!(t.provenance_mem().0 > 0);
    }

    #[test]
    fn off_has_no_provenance() {
        let t = WrittenRefs::default();
        assert_eq!(t.provenance_mem(), (0, 0));
        assert_eq!(t.adl_set_count(), 0);
    }
}
