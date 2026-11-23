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

use crate::ids::{DeclId, ExprId, QualType};
use crate::mem::MemRow;
use crate::types::TypeNames;

#[path = "written_refs_carry.rs"]
mod carry;
pub use carry::{CARRY_SCHEMA_HASH, WrittenRefsSection};

/// The id of a table entry (index + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
pub struct WrittenRefId(NonZeroU32);

impl WrittenRefId {
    /// The entry's index in [`WrittenRefs::iter`] order.
    #[inline]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    fn from_index(i: usize) -> Self {
        // Below the qualifier's global bit (`RefQualifier`).
        let n = u32::try_from(i + 1)
            .ok()
            .filter(|&n| n & GLOBAL_BIT == 0)
            .expect("written-reference table exhausted");
        WrittenRefId(NonZeroU32::new(n).expect("index + 1 is never 0"))
    }
}

/// The tokens of an entry's written name outside its `range`
/// (spec/ast.md §18.11.2, WP-2326): the leading `::` of a name whose
/// qualifier is [`RefQualifier::GLOBAL`] and the `~` of a destructor
/// name, each `Loc::INVALID` when absent. Kept only for the entries that
/// have one, sorted by entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub struct RefLead {
    /// The entry.
    pub entry: WrittenRefId,
    /// Its leading `::` (the global qualifier's token).
    pub colon: Loc,
    /// The `~` before a destructor's class name.
    pub tilde: Loc,
}

/// The syntactic role of a written name (spec/ast.md §18.4). The full
/// set; stage 1 fills `Expr`, `Member` and `Nns`, stage 2 `Type`,
/// `Template`, `Base` and `MemInit`, stage 3 the declarative roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
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
    /// Every role, in declaration order (`r as u8` is its index).
    pub const ALL: [RefRole; 15] = [
        RefRole::Expr,
        RefRole::Member,
        RefRole::Type,
        RefRole::Template,
        RefRole::Nns,
        RefRole::Base,
        RefRole::MemInit,
        RefRole::UsingDecl,
        RefRole::UsingDirective,
        RefRole::NamespaceAlias,
        RefRole::Elaborated,
        RefRole::SpecTarget,
        RefRole::Concept,
        RefRole::Designator,
        RefRole::Redecl,
    ];

    /// The role whose `as u8` is `b` (a module image's byte).
    pub fn from_u8(b: u8) -> Option<RefRole> {
        Self::ALL.get(usize::from(b)).copied()
    }

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
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

/// The bit of a [`RefQualifier`] for a leading `::`: set in
/// [`RefQualifier::GLOBAL`] and in the links [`WrittenRefs::global_link`]
/// hands the parser, whose low bits number the `::` token (an entry id is
/// below it).
const GLOBAL_BIT: u32 = 1 << 31;

/// How far back [`WrittenRefs::repoint`] looks for its entry.
pub const REPOINT_WINDOW: usize = 4096;

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
            n if n & GLOBAL_BIT != 0 => QualLink::Global,
            n => QualLink::Entry(WrittenRefId(NonZeroU32::new(n).expect("n is not 0"))),
        }
    }

    /// The `::` token a link from [`WrittenRefs::global_link`] stands for
    /// (its index in the table's list of them).
    fn colon(self) -> Option<usize> {
        (self.0 & GLOBAL_BIT != 0 && self != RefQualifier::GLOBAL)
            .then_some((self.0 & !GLOBAL_BIT) as usize)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
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
    /// Every kind, in declaration order (`k as u8` is its index).
    pub const ALL: [RefLookup; 8] = [
        RefLookup::None,
        RefLookup::Unqualified,
        RefLookup::Qualified,
        RefLookup::Member,
        RefLookup::Adl,
        RefLookup::Both,
        RefLookup::Builtin,
        RefLookup::Injected,
    ];

    /// The kind whose `as u8` is `b` (a module image's byte).
    pub fn from_u8(b: u8) -> Option<RefLookup> {
        Self::ALL.get(usize::from(b)).copied()
    }

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
pub struct AdlSetId(NonZeroU32);

/// The associated namespaces, classes and enumerations of an
/// argument-dependent lookup ([basic.lookup.argdep]/3) in discovery order
/// (spec/ast.md §18.7.4), interned as one list.
#[derive(Clone, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
struct RefExtra {
    udir: Option<DeclId>,
    adl: Option<AdlSetId>,
}

/// One written name (spec/ast.md §18.2; 28 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
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
    /// Pattern nodes are not remembered: names parsed for the table
    /// alone, in no body and never instantiated (thread-safety attribute
    /// arguments, spec/ast.md §18.8).
    no_nodes: bool,
    /// The dump prefixes each line with the raw location of the name's
    /// start (`#<raw> `): the developer switch `KILN_WREF_RAW`, which the
    /// driver reads from the run's environment (spec/driver.md
    /// "Front-end library entry"). Never carried.
    raw_dump: bool,
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
    /// Stage 4 (§18.9): the per-specialization entries, in recording
    /// order (a dropped one is marked [`SPEC_DEAD`]).
    specs: Vec<SpecRef>,
    /// The pattern node of each dependent entry, for the substituter to
    /// find the entry from the node it rebuilds (§18.9.4).
    spec_nodes: HashMap<PatternNode, WrittenRefId>,
    /// The using-directive through which ordinary lookup at a dependent
    /// entry's definition found a declaration (the entry, the
    /// declaration's first declaration), for the rows of the
    /// specializations that select it (§18.9.1).
    spec_udirs: HashMap<(WrittenRefId, DeclId), DeclId>,
    /// Rebuilt overload sets awaiting overload resolution in an
    /// instantiation: their pattern entry and specialization.
    spec_pending: HashMap<ExprId, (WrittenRefId, DeclId)>,
    /// The live per-specialization entries by content, so one resolution
    /// is kept once (§18.9.1).
    spec_seen: HashMap<SpecRef, u32>,
    /// The per-specialization entry the last overload-resolution patch
    /// made, with the overload set it resolved.
    spec_last: Option<(ExprId, SpecRefId)>,
    /// The specialization the frames below depth `.0` stand for while a
    /// function template specialization's signature is substituted
    /// (§18.9.2): its declaration exists before its frame's entity says.
    spec_key: Option<(u32, DeclId)>,
    /// Stage 5 (§18.10): the sources whose names this table has no
    /// entries for although it is on.
    unrecorded: Vec<Unrecorded>,
    /// The leading `::` and `~` tokens of the entries that have one, by
    /// entry (§18.11.2).
    leads: Vec<RefLead>,
    /// The location of every leading `::` handed out as a link
    /// ([`global_link`](Self::global_link)): scratch of the parse, never
    /// carried (no link outlives the name it qualifies).
    colons: Vec<Loc>,
}

/// Source the table has no entries for although it is on (spec/ast.md
/// §18.10.3): parsed by a compilation that did not record, and brought
/// in from its snapshot or module image.
#[derive(Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub enum Unrecorded {
    /// The prefix of a precompiled header whose snapshot was made with
    /// recording off: no name parsed before the snapshot has an entry,
    /// and no dependent name of its templates has a per-specialization
    /// row. Every file whose [`FileId::index`] is below `files` was
    /// loaded by the prefix (the restoring compilation's own files
    /// follow).
    Prefix {
        /// The number of files the snapshot's source manager held.
        files: u32,
    },
    /// A module unit built from a declaration image written with
    /// recording off: no name written in its files has an entry, and no
    /// dependent name of its templates has a per-specialization row.
    Module {
        /// The module's name (`M`, `M:P`).
        module: String,
        /// The unit's files (the image's file table, the main file first).
        files: Box<[FileId]>,
    },
}

/// A pattern node a dependent entry was made from (spec/ast.md §18.9.4):
/// an expression, or a dependent type at the location of its name (kiln's
/// types are interned and carry no location).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
pub enum PatternNode {
    /// A dependent name expression (`DependentScopeDeclRefExpr`,
    /// `CXXDependentScopeMemberExpr`, a dependent overload set).
    Expr(ExprId),
    /// A dependent type (`typename T::type`, `T::template X<U>`) written
    /// with its name at the location.
    Type(QualType, Loc),
    /// A using-declarator with a dependent qualifier in a class template
    /// (`using T::size;`, `using typename T::type;`): its
    /// `UnresolvedUsingValue` or `UnresolvedUsingTypename` member.
    Decl(DeclId),
}

/// The id of a per-specialization entry (index + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
pub struct SpecRefId(NonZeroU32);

impl SpecRefId {
    /// The entry's index in recording order.
    #[inline]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    fn from_index(i: usize) -> Self {
        let n = u32::try_from(i + 1).expect("per-specialization table exhausted");
        SpecRefId(NonZeroU32::new(n).expect("index + 1 is never 0"))
    }
}

/// [`SpecRef::flags`]: a dropped entry (a SFINAE substitution's, or a
/// duplicate); never seen through the read API.
pub const SPEC_DEAD: u8 = 0x80;

/// What one dependent name of a template pattern resolved to in one
/// specialization (spec/ast.md §18.9; 28 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub struct SpecRef {
    /// The pattern's entry (in the main table, `DEPENDENT`).
    pub entry: WrittenRefId,
    /// The specialization whose instantiation ran the lookup (§18.9.2).
    pub spec: DeclId,
    /// The declaration the name denotes in `spec`.
    pub entity: Option<DeclId>,
    /// What lookup found, when it differs from `entity`.
    pub found: Option<DeclId>,
    /// The argument-dependent set of a call's lookup (§18.7.4).
    pub adl: Option<AdlSetId>,
    /// The using-directive ordinary lookup found the selected declaration
    /// through at the pattern's definition (§18.7.3, §18.9.1).
    pub udir: Option<DeclId>,
    /// [`REF_UNRESOLVED`], [`REF_MEMBER`] (the entry's).
    pub flags: u8,
    /// How lookup found the name (§18.7.2).
    pub how: RefLookup,
}

impl SpecRef {
    /// The `how` the dump leaves out (spec/ast.md §18.7.5), with the
    /// qualifier of the pattern entry `r`.
    pub fn default_how(&self, r: &WrittenRef) -> RefLookup {
        WrittenRef { entity: self.entity, flags: self.flags, ..*r }.default_how()
    }
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

    /// Makes [`dump`](Self::dump) prefix each line with the raw location
    /// of the name's start (`KILN_WREF_RAW`, a developer switch).
    pub fn set_raw_dump(&mut self, raw: bool) {
        self.raw_dump = raw;
    }

    /// The sources this table has no entries for although it is on
    /// (§18.10.3): empty when every name the compilation saw was recorded.
    pub fn unrecorded(&self) -> &[Unrecorded] {
        &self.unrecorded
    }

    /// Notes that the names of `u` were not recorded (nothing when off:
    /// an unrecorded table exposes nothing).
    pub fn note_unrecorded(&mut self, u: Unrecorded) {
        if self.on {
            self.unrecorded.push(u);
        }
    }

    /// Narrows the last [`Unrecorded::Prefix`] row to the `files` the
    /// snapshot's source manager held, once the restoring compilation
    /// knows it (it enters its own files around the restore).
    pub fn set_prefix_files(&mut self, files: u32) {
        if let Some(Unrecorded::Prefix { files: f }) =
            self.unrecorded.iter_mut().rev().find(|u| matches!(u, Unrecorded::Prefix { .. }))
        {
            *f = files;
        }
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

    /// Appends `r`; `None` (and nothing recorded) when off. A qualifier
    /// from [`global_link`](Self::global_link) is stored as
    /// [`RefQualifier::GLOBAL`], with its `::` as the entry's lead.
    pub fn push(&mut self, mut r: WrittenRef) -> Option<WrittenRefId> {
        if !self.on {
            return None;
        }
        let colon = r.qualifier.colon().and_then(|k| self.colons.get(k).copied());
        if r.qualifier.link() == QualLink::Global {
            r.qualifier = RefQualifier::GLOBAL;
        }
        self.entries.push(r);
        let id = WrittenRefId::from_index(self.entries.len() - 1);
        if let Some(c) = colon {
            self.leads.push(RefLead { entry: id, colon: c, tilde: Loc::INVALID });
        }
        Some(id)
    }

    /// The qualifier link of a leading `::` at `colon` (spec/ast.md
    /// §18.11.2): an entry made with it has [`RefQualifier::GLOBAL`] and
    /// [`global_colon`](Self::global_colon) `colon`. Off, plain
    /// [`RefQualifier::GLOBAL`].
    pub fn global_link(&mut self, colon: Loc) -> RefQualifier {
        let k = self.colons.len() as u32;
        if !self.on || k >= GLOBAL_BIT - 1 {
            return RefQualifier::GLOBAL;
        }
        self.colons.push(colon);
        RefQualifier(GLOBAL_BIT | k)
    }

    /// Notes that entry `id` names a destructor whose `~` is at `tilde`
    /// (its `range` is the class name's, §18.11.2).
    pub fn set_tilde(&mut self, id: WrittenRefId, tilde: Loc) {
        self.set_lead(id, None, Some(tilde));
    }

    /// Sets the leading `::` (an entry whose qualifier is
    /// [`RefQualifier::GLOBAL`]) and the `~` of entry `id`, each when
    /// given (a carried table's, §18.10).
    pub fn set_lead(&mut self, id: WrittenRefId, colon: Option<Loc>, tilde: Option<Loc>) {
        let at = match self.leads.last() {
            Some(l) if l.entry < id => self.leads.len(),
            _ => self.leads.partition_point(|l| l.entry < id),
        };
        if self.leads.get(at).is_none_or(|l| l.entry != id) {
            let l = RefLead { entry: id, colon: Loc::INVALID, tilde: Loc::INVALID };
            self.leads.insert(at, l);
        }
        let l = &mut self.leads[at];
        if let Some(c) = colon {
            l.colon = c;
        }
        if let Some(t) = tilde {
            l.tilde = t;
        }
    }

    /// The leading tokens of entry `id` outside its range, when it has
    /// any.
    pub fn lead(&self, id: WrittenRefId) -> Option<&RefLead> {
        let at = self.leads.binary_search_by_key(&id, |l| l.entry).ok()?;
        self.leads.get(at)
    }

    /// Every entry's leading tokens, by entry.
    pub fn leads(&self) -> &[RefLead] {
        &self.leads
    }

    /// The location of the leading `::` of entry `id`, whose qualifier
    /// is [`RefQualifier::GLOBAL`] (`None` for any other entry, and for
    /// one whose `::` was not recorded).
    pub fn global_colon(&self, id: WrittenRefId) -> Option<Loc> {
        self.lead(id).map(|l| l.colon).filter(|l| l.is_valid())
    }

    /// The location of the `~` of entry `id`, a destructor name `~T`
    /// (its `range` is `T`'s).
    pub fn tilde(&self, id: WrittenRefId) -> Option<Loc> {
        self.lead(id).map(|l| l.tilde).filter(|l| l.is_valid())
    }

    /// Where the name of entry `id` begins as an expression stores it:
    /// the `~` of a destructor name, else `range.lo`. The location of a
    /// `DeclRefExpr` or `MemberExpr` built from an id-expression or a
    /// member name equals it (the join, spec/ast.md §18.11.1).
    pub fn name_lo(&self, id: WrittenRefId) -> Loc {
        self.tilde(id).unwrap_or(self.get(id).range.lo)
    }

    /// Where the qualified name ending in entry `id` begins: the leading
    /// `::`, else the first component's `range.lo` (the start of `id`'s
    /// own name, [`name_lo`](Self::name_lo), when it is unqualified). A
    /// `decltype(...)::` component has no entry and is not included.
    pub fn qualified_lo(&self, id: WrittenRefId) -> Loc {
        let mut cur = id;
        // Each qualifier precedes its entry: the walk ends.
        while let QualLink::Entry(q) = self.get(cur).qualifier.link() {
            cur = q;
        }
        match self.get(cur).qualifier.link() {
            QualLink::Global => self.global_colon(cur).unwrap_or(self.get(cur).range.lo),
            _ if cur == id => self.name_lo(id),
            _ => self.get(cur).range.lo,
        }
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

    /// Re-points the newest entry whose name begins at `lo` and names
    /// `old` to `entity` (`found` when it differs), for a name sema
    /// resolved once more after it was recorded (a function template-id
    /// re-resolved against its target, spec/ast.md §18.11.1). The entry
    /// is among the last [`REPOINT_WINDOW`] (the full-expression's);
    /// `None` when it is not.
    pub fn repoint(
        &mut self,
        lo: Loc,
        old: DeclId,
        entity: DeclId,
        found: Option<DeclId>,
    ) -> Option<WrittenRefId> {
        let from = self.entries.len().saturating_sub(REPOINT_WINDOW);
        let i = (from..self.entries.len())
            .rev()
            .find(|&i| self.entries[i].range.lo == lo && self.entries[i].entity == Some(old))?;
        let r = &mut self.entries[i];
        r.entity = Some(entity);
        r.found = found.filter(|&f| f != entity);
        Some(WrittenRefId::from_index(i))
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
        let sid = self.intern_adl(set);
        self.extra.entry(id).or_default().adl = Some(sid);
    }

    /// The interned id of `set` (one copy per distinct set, §18.7.1).
    pub fn intern_adl(&mut self, set: AdlSet) -> AdlSetId {
        if let Some(&s) = self.adl_index.get(&set) {
            return s;
        }
        let n = u32::try_from(self.adl_sets.len() + 1).expect("ADL sets exhausted");
        let s = AdlSetId(NonZeroU32::new(n).expect("index + 1 is never 0"));
        self.adl_sets.push(set.clone());
        self.adl_index.insert(set, s);
        s
    }

    /// The interned set `s`.
    pub fn adl_set_of(&self, s: AdlSetId) -> &AdlSet {
        &self.adl_sets[(s.0.get() - 1) as usize]
    }

    // Stage 4: per-specialization entries (spec/ast.md §18.9).

    /// Remembers that the dependent entry `id` was made from the pattern
    /// node `n` (§18.9.4); nothing when off.
    pub fn note_pattern_node(&mut self, n: PatternNode, id: WrittenRefId) {
        if self.on && !self.no_nodes {
            self.spec_nodes.insert(n, id);
        }
    }

    /// Stops (`false`) or resumes remembering pattern nodes and their
    /// using-directives, for names parsed for the table alone that no
    /// instantiation rebuilds (§18.8); returns the previous setting.
    pub fn set_pattern_nodes(&mut self, on: bool) -> bool {
        !std::mem::replace(&mut self.no_nodes, !on)
    }

    /// True when some dependent entry has a pattern node: the
    /// substituter's one branch (§18.9.4).
    #[inline]
    pub fn has_pattern_nodes(&self) -> bool {
        !self.spec_nodes.is_empty()
    }

    /// Every pattern node and its entry, in no particular order.
    pub fn pattern_nodes(&self) -> impl Iterator<Item = (PatternNode, WrittenRefId)> + '_ {
        self.spec_nodes.iter().map(|(&n, &id)| (n, id))
    }

    /// Remembers that ordinary lookup of dependent entry `id` at its
    /// definition found `decl` (a first declaration) through the
    /// using-directive `udir` (§18.9.1); nothing when off.
    pub fn note_pattern_udir(&mut self, id: WrittenRefId, decl: DeclId, udir: DeclId) {
        if self.on && !self.no_nodes {
            self.spec_udirs.insert((id, decl), udir);
        }
    }

    /// The using-directive ordinary lookup of dependent entry `id` found
    /// `decl` (a first declaration) through at the definition.
    #[inline]
    pub fn pattern_udir(&self, id: WrittenRefId, decl: DeclId) -> Option<DeclId> {
        if self.spec_udirs.is_empty() { None } else { self.spec_udirs.get(&(id, decl)).copied() }
    }

    /// Every (entry, declaration) with its using-directive, in no
    /// particular order.
    pub fn pattern_udirs(&self) -> impl Iterator<Item = ((WrittenRefId, DeclId), DeclId)> + '_ {
        self.spec_udirs.iter().map(|(&k, &v)| (k, v))
    }

    /// The dependent entry made from the pattern node `n`.
    #[inline]
    pub fn pattern_entry(&self, n: PatternNode) -> Option<WrittenRefId> {
        if self.spec_nodes.is_empty() { None } else { self.spec_nodes.get(&n).copied() }
    }

    /// Records `r`, or returns the live entry with the same content
    /// (one resolution is kept once, §18.9.1).
    pub fn push_spec(&mut self, r: SpecRef) -> SpecRefId {
        let r = SpecRef { flags: r.flags & !SPEC_DEAD, ..r };
        if let Some(&i) = self.spec_seen.get(&r) {
            return SpecRefId::from_index(i as usize);
        }
        let i = self.specs.len();
        self.specs.push(r);
        self.spec_seen.insert(r, i as u32);
        SpecRefId::from_index(i)
    }

    /// Changes entry `id` by `f`; when another live entry then has the
    /// same content, `id` is dropped and that one returned.
    pub fn update_spec(&mut self, id: SpecRefId, f: impl FnOnce(&mut SpecRef)) -> SpecRefId {
        let i = id.index();
        let old = self.specs[i];
        if old.flags & SPEC_DEAD != 0 {
            return id;
        }
        let mut new = old;
        f(&mut new);
        new.flags &= !SPEC_DEAD;
        if new == old {
            return id;
        }
        if self.spec_seen.get(&old) == Some(&(i as u32)) {
            self.spec_seen.remove(&old);
        }
        if let Some(&j) = self.spec_seen.get(&new) {
            self.specs[i].flags |= SPEC_DEAD;
            return SpecRefId::from_index(j as usize);
        }
        self.specs[i] = new;
        self.spec_seen.insert(new, i as u32);
        id
    }

    /// The rebuilt overload set `x` of pattern entry `entry` awaits
    /// overload resolution in specialization `spec`.
    pub fn defer_spec(&mut self, x: ExprId, entry: WrittenRefId, spec: DeclId) {
        self.spec_pending.insert(x, (entry, spec));
    }

    /// True when a rebuilt overload set awaits resolution.
    #[inline]
    pub fn has_spec_pending(&self) -> bool {
        !self.spec_pending.is_empty()
    }

    /// The pattern entry and specialization of the rebuilt overload set
    /// `x` while it awaits resolution.
    pub fn spec_pending_for(&self, x: ExprId) -> Option<(WrittenRefId, DeclId)> {
        self.spec_pending.get(&x).copied()
    }

    /// Takes the pending resolution of `x` (it is being resolved).
    pub fn take_spec_pending(&mut self, x: ExprId) -> Option<(WrittenRefId, DeclId)> {
        if self.spec_pending.is_empty() { None } else { self.spec_pending.remove(&x) }
    }

    /// The rebuilt overload set `from` goes on as `to`.
    pub fn retarget_spec(&mut self, from: ExprId, to: ExprId) {
        if let Some(p) = self.spec_pending.remove(&from) {
            self.spec_pending.insert(to, p);
        }
    }

    /// Remembers that resolving the overload set `x` made entry `id`.
    pub fn set_spec_last(&mut self, x: ExprId, id: SpecRefId) {
        self.spec_last = Some((x, id));
    }

    /// The entry resolving the overload set `x` made, once.
    pub fn take_spec_last(&mut self, x: ExprId) -> Option<SpecRefId> {
        match self.spec_last.take() {
            Some((e, id)) if e == x => Some(id),
            _ => None,
        }
    }

    /// Sets (or clears, returning the previous one) the specialization
    /// the instantiation frames below depth `depth` stand for (§18.9.2).
    pub fn set_spec_key(&mut self, k: Option<(u32, DeclId)>) -> Option<(u32, DeclId)> {
        std::mem::replace(&mut self.spec_key, k)
    }

    /// The specialization set by [`set_spec_key`](Self::set_spec_key).
    #[inline]
    pub fn spec_key(&self) -> Option<(u32, DeclId)> {
        self.spec_key
    }

    /// The specializations of the live entries recorded since `mark`.
    pub fn spec_keys_since(&self, mark: u32) -> Vec<DeclId> {
        let mut v: Vec<DeclId> = self.specs[(mark as usize).min(self.specs.len())..]
            .iter()
            .filter(|r| r.flags & SPEC_DEAD == 0)
            .map(|r| r.spec)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The position of the per-specialization table, for
    /// [`drop_specs_since`](Self::drop_specs_since).
    #[inline]
    pub fn spec_mark(&self) -> u32 {
        self.specs.len() as u32
    }

    /// Drops the entries recorded since `mark` whose specialization
    /// `drop` selects (a SFINAE substitution's, §18.9.3).
    pub fn drop_specs_since(&mut self, mark: u32, drop: impl Fn(DeclId) -> bool) {
        for i in mark as usize..self.specs.len() {
            let r = self.specs[i];
            if r.flags & SPEC_DEAD == 0 && drop(r.spec) {
                if self.spec_seen.get(&r) == Some(&(i as u32)) {
                    self.spec_seen.remove(&r);
                }
                self.specs[i].flags |= SPEC_DEAD;
            }
        }
    }

    /// Every live per-specialization entry, in recording order.
    pub fn specs(&self) -> impl Iterator<Item = (SpecRefId, &SpecRef)> + '_ {
        self.specs
            .iter()
            .enumerate()
            .filter(|(_, r)| r.flags & SPEC_DEAD == 0)
            .map(|(i, r)| (SpecRefId::from_index(i), r))
    }

    /// The per-specialization entry `id`.
    pub fn spec(&self, id: SpecRefId) -> &SpecRef {
        &self.specs[id.index()]
    }

    /// The number of live per-specialization entries.
    pub fn spec_len(&self) -> usize {
        self.spec_seen.len()
    }

    /// The per-specialization entries of pattern entry `id`.
    pub fn specs_of(&self, id: WrittenRefId) -> impl Iterator<Item = (SpecRefId, &SpecRef)> + '_ {
        self.specs().filter(move |(_, r)| r.entry == id)
    }

    /// The per-specialization entries of specialization `d`.
    pub fn specs_for(&self, d: DeclId) -> impl Iterator<Item = (SpecRefId, &SpecRef)> + '_ {
        self.specs().filter(move |(_, r)| r.spec == d)
    }

    /// The per-specialization entries that name `d`, as their entity or
    /// as the declaration lookup found: with the main table's entries
    /// naming `d`, every written occurrence a rename of `d` changes
    /// (§18.9.1).
    pub fn specs_naming(&self, d: DeclId) -> impl Iterator<Item = (SpecRefId, &SpecRef)> + '_ {
        self.specs().filter(move |(_, r)| r.entity == Some(d) || r.found == Some(d))
    }

    /// The live per-specialization entries grouped by pattern entry, for
    /// clients that query many entries (one pass).
    pub fn specs_by_entry(&self) -> HashMap<WrittenRefId, Vec<SpecRefId>> {
        let mut m: HashMap<WrittenRefId, Vec<SpecRefId>> = HashMap::new();
        for (id, r) in self.specs() {
            m.entry(r.entry).or_default().push(id);
        }
        m
    }

    /// A per-specialization entry's argument-dependent set (§18.7.4).
    pub fn spec_adl_set(&self, r: &SpecRef) -> Option<&AdlSet> {
        r.adl.map(|s| self.adl_set_of(s))
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
        self.leads.retain(|l| l.entry.index() < n);
        self.pending.retain(|_, id| id.index() < n);
        self.extra.retain(|id, _| id.index() < n);
        self.designators.retain(|_, id| id.index() < n);
        self.spec_nodes.retain(|_, id| id.index() < n);
        self.spec_udirs.retain(|(id, _), _| id.index() < n);
        self.spec_pending.retain(|_, (id, _)| id.index() < n);
        for r in &mut self.specs {
            if r.entry.index() >= n {
                r.flags |= SPEC_DEAD;
            }
        }
        self.spec_seen.retain(|r, _| r.entry.index() < n);
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
        let lead = size_of::<RefLead>() as u64;
        let colon = size_of::<Loc>() as u64;
        let (prov_live, prov_reserved) = self.provenance_mem();
        let (spec_live, spec_reserved) = self.spec_mem();
        Some(MemRow::new(
            "written_refs",
            self.entries.len() as u64,
            self.entries.len() as u64 * sz
                + self.pending.len() as u64 * pend
                + self.designators.len() as u64 * desig
                + self.leads.len() as u64 * lead
                + self.colons.len() as u64 * colon
                + prov_live
                + spec_live,
            self.entries.capacity() as u64 * sz
                + self.pending.capacity() as u64 * pend
                + self.designators.capacity() as u64 * desig
                + self.leads.capacity() as u64 * lead
                + self.colons.capacity() as u64 * colon
                + prov_reserved
                + spec_reserved,
        ))
    }

    /// The bytes (live, reserved) of stage 4 (§18.9.6): the entries, the
    /// pattern-node map, the pending overload sets and the content index
    /// (a hash map slot counted twice its pair, as in
    /// [`provenance_mem`](Self::provenance_mem)).
    pub fn spec_mem(&self) -> (u64, u64) {
        let row = size_of::<SpecRef>() as u64;
        let node = (size_of::<(PatternNode, WrittenRefId)>() as u64) * 2;
        let pend = (size_of::<(ExprId, (WrittenRefId, DeclId))>() as u64) * 2;
        let seen = (size_of::<(SpecRef, u32)>() as u64) * 2;
        let udir = (size_of::<((WrittenRefId, DeclId), DeclId)>() as u64) * 2;
        let live = self.specs.len() as u64 * row
            + self.spec_udirs.len() as u64 * udir
            + self.spec_nodes.len() as u64 * node
            + self.spec_pending.len() as u64 * pend
            + self.spec_seen.len() as u64 * seen;
        let reserved = self.specs.capacity() as u64 * row
            + self.spec_udirs.capacity() as u64 * udir
            + self.spec_nodes.capacity() as u64 * node
            + self.spec_pending.capacity() as u64 * pend
            + self.spec_seen.capacity() as u64 * seen;
        (live, reserved)
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

    /// The dump's first lines: the sources whose names were not recorded
    /// (§18.10.3).
    fn dump_unrecorded(&self, sm: &SourceManager, out: &mut String) {
        for u in &self.unrecorded {
            match u {
                Unrecorded::Prefix { files } => {
                    let _ = writeln!(out, "unrecorded  precompiled prefix ({files} files)");
                }
                Unrecorded::Module { module, files } => {
                    let main = files.first().map_or("?", |&f| {
                        let n = sm.file_name(f);
                        n.rsplit(['/', '\\']).next().unwrap_or(n)
                    });
                    let _ = writeln!(
                        out,
                        "unrecorded  module {module} ({main}, {} files)",
                        files.len()
                    );
                }
            }
        }
    }

    /// The dump's `q=` and `tilde=` fields of entry `id` (§18.6).
    fn dump_lead(&self, id: WrittenRefId, out: &mut String, pos: &dyn Fn(Loc) -> String) {
        match self.get(id).qualifier.link() {
            QualLink::None => {}
            QualLink::Global => {
                out.push_str(" q=::");
                if let Some(c) = self.global_colon(id) {
                    out.push_str(&pos(c));
                }
            }
            QualLink::Entry(q) => {
                out.push_str(" q=");
                out.push_str(&pos(self.get(q).range.lo));
            }
        }
        if let Some(t) = self.tilde(id) {
            out.push_str(" tilde=");
            out.push_str(&pos(t));
        }
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
        let decl_in = |names: &crate::diag_names::DiagNames<'_>, d: DeclId, out: &mut String| {
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
        let decl = |d: DeclId, out: &mut String| decl_in(&names, d, out);
        // A per-specialization line prints its specializations with their
        // canonical arguments, whichever spelling instantiated them
        // first (§18.9.5).
        let canon = names.canonical_args();
        let canon_decl = |d: DeclId, out: &mut String| decl_in(&canon, d, out);
        let mut out = String::new();
        self.dump_unrecorded(sm, &mut out);
        // The per-specialization entries of each pattern entry (§18.9.5).
        let mut by_entry: HashMap<WrittenRefId, Vec<&SpecRef>> = HashMap::new();
        for (_, r) in self.specs() {
            by_entry.entry(r.entry).or_default().push(r);
        }
        for (i, r) in self.entries.iter().enumerate() {
            let id = WrittenRefId::from_index(i);
            let (lf, lo) = pos2(r.range.lo);
            let (hf, hi) = pos2(r.range.hi);
            let hi = if hf == lf { hi } else { hf + &hi };
            let lo = lf + &lo;
            let text = sm.span_text(r.range).unwrap_or("?");
            if self.raw_dump {
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
            self.dump_lead(id, &mut out, &pos);
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
            // Its per-specialization lines, sorted: the order of
            // instantiation does not show (§18.9.5).
            if let Some(v) = by_entry.get(&id) {
                let head = format!("{lo}-{hi}  {}  '{text}' -> ", r.role.spelling());
                self.dump_specs(r, v, &head, ast, &canon, &pos, &canon_decl, &mut out);
            }
        }
        out
    }

    /// The per-specialization lines of entry `r` (`rows`), each starting
    /// with `head` (the entry's positions, role and spelling), sorted and
    /// without duplicates (§18.9.5).
    #[allow(clippy::too_many_arguments, reason = "the dump's shared printers")]
    fn dump_specs(
        &self,
        r: &WrittenRef,
        rows: &[&SpecRef],
        head: &str,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        decl: &dyn Fn(DeclId, &mut String),
        out: &mut String,
    ) {
        // A specialization prints with its arguments, a class's and a
        // variable's too (`Table<Cfg>`, `scale_of<Feet>`).
        let spec_decl = |d: DeclId, out: &mut String| {
            let mut s = String::new();
            names.write_decl_name(d, true, &mut s);
            if !ast.decls.get(d).kind().is_function()
                && let Some((_, info)) = ast.spec_info(d)
            {
                names.write_spec_args(names, d, info.print_args, &mut s);
            }
            if s.is_empty() {
                s.push_str("(anonymous)");
            }
            out.push_str(&s);
            out.push('@');
            out.push_str(&pos(ast.decls.get(d).loc()));
        };
        let mut lines: Vec<String> = rows
            .iter()
            .map(|sr| {
                let mut l = head.to_owned();
                match sr.entity {
                    Some(d) => decl(d, &mut l),
                    None => l.push_str("(unresolved)"),
                }
                if let Some(f) = sr.found {
                    l.push_str(" found=");
                    decl(f, &mut l);
                }
                l.push_str(" spec=");
                spec_decl(sr.spec, &mut l);
                if sr.how != sr.default_how(r) {
                    l.push_str(" how=");
                    l.push_str(sr.how.spelling());
                }
                if let Some(u) = sr.udir {
                    dump_udir(u, ast, names, pos, &mut l);
                }
                if let Some(set) = self.spec_adl_set(sr) {
                    dump_adl(set, ast, pos, decl, &mut l);
                }
                l.push('\n');
                l
            })
            .collect();
        lines.sort();
        lines.dedup();
        for l in lines {
            out.push_str(&l);
        }
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
            dump_udir(u, ast, names, pos, out);
        }
        if let Some(set) = self.adl_set(id) {
            dump_adl(set, ast, pos, decl, out);
        }
    }
}

/// The `udir=` field of a dump line (spec/ast.md §18.7.5): the
/// nominated namespace at the directive's location.
fn dump_udir(
    u: DeclId,
    ast: &crate::Ast,
    names: &crate::diag_names::DiagNames<'_>,
    pos: &dyn Fn(Loc) -> String,
    out: &mut String,
) {
    out.push_str(" udir=");
    if let Some(n) = ast.using_target(u) {
        names.write_decl_name(n, true, out);
    }
    out.push('@');
    out.push_str(&pos(ast.decls.get(u).loc()));
}

/// The `adl-ns=`, `adl-cls=` and `adl-enum=` fields of a dump line
/// (spec/ast.md §18.7.5).
fn dump_adl(
    set: &AdlSet,
    ast: &crate::Ast,
    pos: &dyn Fn(Loc) -> String,
    decl: &dyn Fn(DeclId, &mut String),
    out: &mut String,
) {
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

    fn at(lo: u32, q: RefQualifier) -> WrittenRef {
        let lo = Loc::from_raw(lo);
        WrittenRef { range: Span::new(lo, lo), qualifier: q, ..entry(None) }
    }

    #[test]
    fn leading_colons_and_tildes_are_kept_beside_the_entry() {
        let mut t = WrittenRefs::default();
        // Off: the plain global link, nothing kept.
        assert_eq!(t.global_link(Loc::from_raw(3)), RefQualifier::GLOBAL);
        assert!(t.colons.is_empty());
        t.set_on(true);
        // `::a::b`: the link `a` is made with is stored as GLOBAL.
        let link = t.global_link(Loc::from_raw(10));
        assert_eq!(link.link(), QualLink::Global);
        let ea = t.push(at(12, link)).unwrap();
        let eb = t.push(at(15, RefQualifier::entry(ea))).unwrap();
        assert_eq!(t.get(ea).qualifier, RefQualifier::GLOBAL);
        assert_eq!(t.global_colon(ea), Some(Loc::from_raw(10)));
        assert_eq!(t.global_colon(eb), None);
        assert_eq!(t.qualified_lo(eb), Loc::from_raw(10));
        assert_eq!(t.name_lo(eb), Loc::from_raw(15));
        // A GLOBAL link of no recorded `::` (a carried table's) has none.
        let ec = t.push(at(20, RefQualifier::GLOBAL)).unwrap();
        assert_eq!(t.global_colon(ec), None);
        assert_eq!(t.qualified_lo(ec), Loc::from_raw(20));
        // `x.~T()`: the join location is the `~`.
        let mark = t.mark(0);
        let dtor = t.push(at(31, RefQualifier::NONE)).unwrap();
        t.set_tilde(dtor, Loc::from_raw(30));
        assert_eq!(t.tilde(dtor), Some(Loc::from_raw(30)));
        assert_eq!(t.name_lo(dtor), Loc::from_raw(30));
        assert_eq!(t.qualified_lo(dtor), Loc::from_raw(30));
        // Out-of-order lead rows (a carried table's) stay sorted.
        t.set_lead(ec, None, Some(Loc::from_raw(19)));
        let entries: Vec<_> = t.leads().iter().map(|l| l.entry).collect();
        assert_eq!(entries, [ea, ec, dtor]);
        assert_eq!(t.tilde(ec), Some(Loc::from_raw(19)));
        t.truncate(mark);
        assert_eq!(t.leads().len(), 2);
        assert_eq!(t.tilde(dtor), None);
        assert_eq!(size_of::<WrittenRef>(), 28);
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

    fn spec_row(entry: WrittenRefId, spec: u32, entity: u32) -> SpecRef {
        SpecRef {
            entry,
            spec: DeclId::new(spec),
            entity: Some(DeclId::new(entity)),
            found: None,
            adl: None,
            udir: None,
            flags: 0,
            how: RefLookup::Member,
        }
    }

    #[test]
    fn spec_row_is_28_bytes() {
        assert_eq!(size_of::<SpecRef>(), 28);
        assert_eq!(size_of::<Option<SpecRefId>>(), 4);
    }

    #[test]
    fn off_records_no_pattern_nodes() {
        let mut t = WrittenRefs::default();
        t.note_pattern_node(PatternNode::Expr(ExprId::new(3)), WrittenRefId::from_index(0));
        t.note_pattern_udir(WrittenRefId::from_index(0), DeclId::new(3), DeclId::new(4));
        assert!(!t.has_pattern_nodes());
        assert_eq!(t.pattern_udir(WrittenRefId::from_index(0), DeclId::new(3)), None);
        assert_eq!(t.pattern_entry(PatternNode::Expr(ExprId::new(3))), None);
        assert_eq!(t.spec_mem(), (0, 0));
    }

    #[test]
    fn spec_rows_are_kept_once() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let w = t.push(entry(None)).expect("on");
        let a = t.push_spec(spec_row(w, 10, 20));
        let b = t.push_spec(spec_row(w, 11, 21));
        // The same resolution again (a second instantiation path).
        assert_eq!(t.push_spec(spec_row(w, 10, 20)), a);
        assert_eq!(t.spec_len(), 2);
        // Updated to equal another row: merged into it.
        assert_eq!(t.update_spec(b, |r| *r = spec_row(w, 10, 20)), a);
        assert_eq!(t.spec_len(), 1);
        assert_eq!(t.specs().count(), 1);
        assert_eq!(t.specs_naming(DeclId::new(20)).count(), 1);
        assert_eq!(t.specs_of(w).count(), 1);
    }

    #[test]
    fn sfinae_drop_and_truncate() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let w0 = t.push(entry(None)).expect("on");
        let w1 = t.push(entry(None)).expect("on");
        t.push_spec(spec_row(w0, 10, 20));
        let mark = t.spec_mark();
        t.push_spec(spec_row(w0, 11, 21));
        t.push_spec(spec_row(w1, 12, 22));
        assert_eq!(t.spec_keys_since(mark), [DeclId::new(11), DeclId::new(12)]);
        // A failed substitution for 11 drops its rows only.
        t.drop_specs_since(mark, |k| k == DeclId::new(11));
        let left: Vec<_> = t.specs().map(|(_, r)| r.spec).collect();
        assert_eq!(left, [DeclId::new(10), DeclId::new(12)]);
        // Recorded again after the drop: a new live row.
        t.push_spec(spec_row(w0, 11, 21));
        assert_eq!(t.spec_len(), 3);
        // Rolling the main table back drops the rows of its entries.
        t.note_pattern_node(PatternNode::Expr(ExprId::new(7)), w1);
        t.note_pattern_udir(w0, DeclId::new(30), DeclId::new(31));
        t.note_pattern_udir(w1, DeclId::new(30), DeclId::new(32));
        assert_eq!(t.pattern_udir(w1, DeclId::new(30)), Some(DeclId::new(32)));
        t.truncate(RefMark { entries: 1, decls: 0 });
        assert_eq!(t.pattern_entry(PatternNode::Expr(ExprId::new(7))), None);
        assert_eq!(t.pattern_udir(w1, DeclId::new(30)), None);
        assert_eq!(t.pattern_udir(w0, DeclId::new(30)), Some(DeclId::new(31)));
        assert!(t.specs().all(|(_, r)| r.entry == w0));
        assert_eq!(t.spec_len(), 2);
    }
}
