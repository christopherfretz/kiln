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

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::num::NonZeroU32;

use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::ids::{ArgListId, DeclId, ExprId, QualType, RawId};
use crate::mem::MemRow;
use crate::types::TypeNames;

#[path = "written_refs_carry.rs"]
mod carry;
#[path = "written_refs_subst.rs"]
mod subst;
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
/// qualifier is [`RefQualifier::GLOBAL`], the `~` of a destructor name,
/// the first token of the specifier before a name whose qualifier is
/// [`RefQualifier::SPECIFIER`] (WP-2828), the closing `]` or `)` of a
/// subscript or call operator entry (§18.13, WP-2820) and the `<` and `>`
/// of a friend function template specialization's template argument list
/// (WP-3008), each `Loc::INVALID` when absent. Kept only for the entries
/// that have one, sorted by entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub struct RefLead {
    /// The entry.
    pub entry: WrittenRefId,
    /// Its leading `::` (the global qualifier's token).
    pub colon: Loc,
    /// The `~` before a destructor's class name.
    pub tilde: Loc,
    /// The first token of the `decltype(...)::` or splice
    /// (`[: r :]::`, `template [: r :]<...>::`) specifier the name
    /// follows: the `decltype` keyword, the `[:`, the `template`.
    pub spec: Loc,
    /// The `]` or `)` closing the `[` or `(` of an `operator` entry.
    pub close: Loc,
    /// The `<` and `>` of the template argument list after a friend
    /// function template specialization's name (`friend void f<>(T);`).
    pub langle: Loc,
    pub rangle: Loc,
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
    /// An operator token whose overload resolution selected a function:
    /// `s [+] s`, `m[[]1]`, `f[(]x)` calling `operator()`, `p[->]m`
    /// through `operator->` (spec/ast.md §18.13, stage 6, WP-2820).
    Operator,
    /// The ud-suffix of a user-defined literal, naming its literal
    /// operator: `12[_k]`, `"ab"[_s]` (§18.13, stage 6).
    UdSuffix,
    /// A written use of a label: `goto [l]`, `&&[l]`, an `asm goto` label
    /// operand (§18.4, WP-3688).
    Label,
}

impl RefRole {
    /// Every role, in declaration order (`r as u8` is its index).
    pub const ALL: [RefRole; 18] = [
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
        RefRole::Operator,
        RefRole::UdSuffix,
        RefRole::Label,
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
            RefRole::Operator => "operator",
            RefRole::UdSuffix => "ud-suffix",
            RefRole::Label => "label",
        }
    }
}

/// The `qualifier` link of an entry: none, the global scope (a leading
/// `::`), a specifier with no entry (`decltype(e)::`, a splice), or the
/// entry of the component to the name's left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
pub struct RefQualifier(u32);

/// [`RefQualifier`] decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualLink {
    /// An unqualified name.
    None,
    /// A name after a leading `::`.
    Global,
    /// A name after a `decltype(...)::` or splice-specifier `[: r :]::`
    /// (`template [: r :]<...>::`), which has no entry of its own (its
    /// operand's names have theirs); [`WrittenRefs::specifier_lo`] is
    /// its first token (spec/ast.md §18.11.2, WP-2828).
    Specifier,
    /// A name after the component recorded as this entry.
    Entry(WrittenRefId),
}

/// The bit of a [`RefQualifier`] for a link with no component entry: set
/// in [`RefQualifier::GLOBAL`], [`RefQualifier::SPECIFIER`] and in the
/// links [`WrittenRefs::global_link`] and
/// [`WrittenRefs::specifier_link`] hand the parser, whose low bits
/// number the token (an entry id is below it).
const GLOBAL_BIT: u32 = 1 << 31;
/// With [`GLOBAL_BIT`]: the link of a specifier, not of a `::`.
const SPEC_BIT: u32 = 1 << 30;

/// How far back [`WrittenRefs::repoint`] looks for its entry.
pub const REPOINT_WINDOW: usize = 4096;

impl RefQualifier {
    /// An unqualified name.
    pub const NONE: RefQualifier = RefQualifier(0);
    /// A name after a leading `::`.
    pub const GLOBAL: RefQualifier = RefQualifier(u32::MAX);
    /// A name after a `decltype(...)::` or splice specifier (no entry).
    pub const SPECIFIER: RefQualifier = RefQualifier(u32::MAX - 1);

    /// A name after the component recorded as `id`.
    pub const fn entry(id: WrittenRefId) -> RefQualifier {
        RefQualifier(id.0.get())
    }

    /// The link decoded.
    pub fn link(self) -> QualLink {
        match self.0 {
            0 => QualLink::None,
            u32::MAX => QualLink::Global,
            n if n & (GLOBAL_BIT | SPEC_BIT) == GLOBAL_BIT | SPEC_BIT => QualLink::Specifier,
            n if n & GLOBAL_BIT != 0 => QualLink::Global,
            n => QualLink::Entry(WrittenRefId(NonZeroU32::new(n).expect("n is not 0"))),
        }
    }

    /// The token a link from [`WrittenRefs::global_link`] or
    /// [`WrittenRefs::specifier_link`] stands for (its index in the
    /// table's list of them).
    fn colon(self) -> Option<usize> {
        (self.0 & GLOBAL_BIT != 0
            && self != RefQualifier::GLOBAL
            && self != RefQualifier::SPECIFIER)
            .then_some((self.0 & !(GLOBAL_BIT | SPEC_BIT)) as usize)
    }
}

/// [`WrittenRef::flags`]: a dependent name in a template pattern.
pub const REF_DEPENDENT: u8 = 1;
/// [`WrittenRef::flags`]: lookup or overload resolution failed.
pub const REF_UNRESOLVED: u8 = 2;
/// [`WrittenRef::flags`]: a member or member template name written after
/// `.` or `->` (spec/ast.md §18.7.2).
pub const REF_MEMBER: u8 = 4;
/// [`WrittenRef::flags`]: an `operator` entry whose overload resolution
/// selected a rewritten comparison candidate ([over.match.oper]/3.4):
/// `a != b` selecting `operator==`, `a < b` selecting `operator<=>` (and
/// the comparison of its result with zero), any reversed candidate
/// (spec/ast.md §18.13.3).
pub const REF_REWRITTEN: u8 = 8;
/// [`WrittenRef::flags`]: the selected candidate is the reversed
/// synthesized form ([over.match.oper]/3.4.4); always with
/// [`REF_REWRITTEN`].
pub const REF_REVERSED: u8 = 16;
/// [`WrittenRef::flags`]: a name in the decl-specifier-seq (or the leading
/// attributes) shared by a declarator group of two or more declarators
/// (`S a, b;`): `enclosing` is the group's first declaration, and every
/// declaration of the group holds the name
/// ([`WrittenRefs::group`], [`WrittenRefs::holders`]; spec/ast.md §18.3.1).
pub const REF_GROUP: u8 = 32;
/// [`WrittenRef::flags`]: an `operator` entry whose overload resolution ran
/// (an operand had class or enumeration type, [over.match.oper]/1-2) and
/// selected a built-in candidate ([over.built]; spec/ast.md §18.13, WP-3701).
/// `entity` and `found` are `None`, `how` is [`RefLookup::Builtin`]; dumped
/// `(builtin)`. Distinguishes a selected built-in candidate from
/// [`REF_UNRESOLVED`].
pub const REF_BUILTIN: u8 = 64;

/// A declarator group of two or more declarators (spec/ast.md §18.3.1):
/// a run of [`WrittenRefs`]' group declarations, the first of which an
/// entry with [`REF_GROUP`] has as its `enclosing`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub struct DeclGroupRec {
    /// The run's start in the group declarations.
    pub start: u32,
    /// The number of declarations (2 or more).
    pub len: u32,
}

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

/// One element of a dependent entry's found set (spec/ast.md §18.12;
/// 8 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub struct FoundDecl {
    /// The entity's first declaration: what a using-declaration names,
    /// or a dependent using-declarator's `UnresolvedUsingValue` member.
    pub decl: DeclId,
    /// The using-shadow lookup found, else `decl`.
    pub found: DeclId,
}

/// The id of an interned found set (index + 1): equal ids are equal sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
pub struct FoundSetId(NonZeroU32);

impl FoundSetId {
    /// The set's index in interning order.
    #[inline]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    fn from_index(i: usize) -> Self {
        let n = u32::try_from(i + 1).expect("found sets exhausted");
        FoundSetId(NonZeroU32::new(n).expect("index + 1 is never 0"))
    }
}

/// [`FoundSet::flags`]: argument-dependent lookup at each instantiation
/// adds to the set ([temp.dep.candidate]/1).
pub const FOUND_ADL: u8 = 1;
/// [`FoundSet::flags`]: the class searched has a dependent base and does
/// not declare the name itself, so the instantiation's lookup can find
/// more ([temp.dep.type], [class.member.lookup]/2).
pub const FOUND_DEPENDENT_BASE: u8 = 2;
/// [`FoundSet::flags`]: an element is a dependent using-declarator,
/// whose target exists per specialization only.
pub const FOUND_UNRESOLVED_USING: u8 = 4;

/// The found set of a dependent entry (spec/ast.md §18.12): what the
/// lookup at the template definition found, and how an instantiation can
/// leave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundSet<'a> {
    /// The elements, in the order overload resolution enumerates them
    /// (innermost scope first, newest first); clients treat them as a set.
    pub decls: &'a [FoundDecl],
    /// The definition lookup's kind: `Unqualified`, `Qualified` or
    /// `Member` (§18.7.2).
    pub how: RefLookup,
    /// [`FOUND_ADL`], [`FOUND_DEPENDENT_BASE`], [`FOUND_UNRESOLVED_USING`].
    pub flags: u8,
}

impl FoundSet<'_> {
    /// True when every instantiation selects from the set (no flag).
    pub fn is_closed(&self) -> bool {
        self.flags == 0
    }

    /// True when argument-dependent lookup adds to the set at
    /// instantiation.
    pub fn adl(&self) -> bool {
        self.flags & FOUND_ADL != 0
    }

    /// True when the class searched has a dependent base that could
    /// supply the name: the class does not declare it itself
    /// (spec/ast.md §18.12.2).
    pub fn dependent_base(&self) -> bool {
        self.flags & FOUND_DEPENDENT_BASE != 0
    }

    /// True when an element is a dependent using-declarator.
    pub fn unresolved_using(&self) -> bool {
        self.flags & FOUND_UNRESOLVED_USING != 0
    }

    /// True when lookup found nothing.
    pub fn is_empty(&self) -> bool {
        self.decls.is_empty()
    }

    /// True when `d` is an element's `decl` or `found`.
    pub fn contains(&self, d: DeclId) -> bool {
        self.decls.iter().any(|e| e.decl == d || e.found == d)
    }
}

/// An interned found set: its run of `found_decls`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
struct FoundSetRec {
    start: u32,
    len: u32,
    how: RefLookup,
    flags: u8,
}

/// The interning key of a found set.
type FoundKey = (RefLookup, u8, Box<[FoundDecl]>);

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
    /// [`REF_DEPENDENT`], [`REF_UNRESOLVED`], [`REF_MEMBER`],
    /// [`REF_REWRITTEN`], [`REF_REVERSED`].
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
    /// The found set of each dependent entry whose definition lookup ran
    /// (§18.12), by entry: sorted by construction (the row is made with
    /// its entry).
    found_rows: Vec<(WrittenRefId, FoundSetId)>,
    /// The interned found sets, by `FoundSetId` index: runs of
    /// `found_decls`.
    found_sets: Vec<FoundSetRec>,
    found_decls: Vec<FoundDecl>,
    /// The found sets by content (rebuilt after a restore).
    found_index: HashMap<FoundKey, FoundSetId>,
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
    /// The naming class ([class.access.base]/5) of the most recent
    /// non-overloaded member or qualified-member expression built while
    /// pattern nodes are recorded, keyed by the built expression
    /// (transient, never carried). Overwriting an unconsumed entry is
    /// expected because non-dependent and sema-synthesized member accesses
    /// have no `PatternNode::Expr`; [`take_last_naming`](Self::take_last_naming)
    /// matches `ExprId` and clears the slot so a stale value never attaches
    /// to another expression.
    last_naming: Option<(ExprId, DeclId)>,
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
    /// The declarator groups whose shared names have entries with
    /// [`REF_GROUP`] (§18.3.1), sorted by their first declaration: runs
    /// of `group_decls`.
    groups: Vec<DeclGroupRec>,
    group_decls: Vec<DeclId>,
    /// The substitution keys and their rows (§18.14): empty, and never
    /// allocated, unless `-fkiln-written-ref-keys` is on.
    subst: SubstTable,
}

/// The keyed table of spec/ast.md §18.14, part of [`WrittenRefs`].
#[derive(Clone, Debug, Default)]
struct SubstTable {
    /// The sub-switch (`-fkiln-written-ref-keys`); effective only while
    /// the table is on.
    on: bool,
    /// The committed keys, in commit order; each names a run of `rows`.
    keys: Vec<SubstKey>,
    /// The keyed rows, one contiguous run per key.
    rows: Vec<SubstRef>,
    /// Per key (parallel to `keys`), its root: start and length in
    /// `root_slots` (§18.14.10).
    root_spans: Vec<(u32, u32)>,
    /// The roots' slots, one contiguous run per key.
    root_slots: Vec<RootSlot>,
    /// Per kind (`SubstKind as usize - 1`), the recording runs that
    /// closed without committing (this parse's own, never carried).
    not_completed: [u32; 4],
    /// The keys by identity (rebuilt after a restore).
    index: HashMap<SubstIdentity, SubstKeyId>,
    /// The pattern nodes reached through an alias template-id kept only
    /// as its expansion (§18.14.2, [`SUBST_EXPANDED`]).
    expanded_nodes: HashSet<PatternNode>,
    /// Rebuilt overload sets awaiting overload resolution inside a key
    /// scope: their pattern entry and scope (transient, as
    /// `spec_pending`).
    pending: HashMap<ExprId, (WrittenRefId, ScopeRef)>,
    /// The keyed row the last overload-resolution patch made: the
    /// overload set, the scope, the row's index in the scope's buffer
    /// (transient, as `spec_last`).
    last: Option<(ExprId, ScopeRef, u32)>,
}

/// A key scope of the substituter (spec/ast.md §18.14.3) as the
/// deferred-overload scratch holds it: the scope's position on the
/// scope stack, a serial that tells it from a later scope at the same
/// position, and the flags its row gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeRef {
    /// The scope's index on the scope stack.
    pub index: u32,
    /// The scope's serial number.
    pub serial: u32,
    /// [`SUBST_EXPANDED`] when the row gets it.
    pub flags: u8,
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
    /// The snapshot was made with the table but without substitution keys
    /// (§18.14), and the consumer records keys: nothing the prefix
    /// substituted has a key. Files as [`Unrecorded::Prefix`].
    PrefixKeys {
        /// The number of files the snapshot's source manager held.
        files: u32,
    },
    /// A module unit built from a declaration image made with the table
    /// but without substitution keys.
    ModuleKeys {
        /// The module's name (`M`, `M:P`).
        module: String,
        /// The unit's files (the image's file table, the main file first).
        files: Box<[FileId]>,
    },
    /// A module unit whose image was made with keys left some out: it
    /// could not carry a key's template, parameter or argument list, or a
    /// row's entry, entity or set member. A key is carried whole or not
    /// at all.
    ModuleKeysLeftOut {
        /// The module's name (`M`, `M:P`).
        module: String,
        /// The whole keys left out.
        keys: u32,
        /// The rows those keys held.
        rows: u32,
    },
}

impl Unrecorded {
    /// True for a gap in the keyed table only (§18.14.5): the main table
    /// and its per-specialization rows are whole.
    pub fn keys_only(&self) -> bool {
        matches!(
            self,
            Unrecorded::PrefixKeys { .. }
                | Unrecorded::ModuleKeys { .. }
                | Unrecorded::ModuleKeysLeftOut { .. }
        )
    }
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
    /// The operator token of a dependent operator expression (spec/ast.md
    /// §18.13.6): the expression (a dependent operator, subscript, call or
    /// member access through `->`, a fold-expression) and the token's
    /// index in it (1 for a binary fold's second operator, else 0). Kept
    /// apart from [`PatternNode::Expr`]: a dependent `t->m` is the node of
    /// both the `->` and the member name.
    Oper(ExprId, u8),
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

/// [`SpecRef::flags`]: the dependent operator token of the row's entry
/// selected no function in this specialization: overload resolution chose
/// a built-in candidate ([over.built]), the operand was a pointer or a
/// function pointer (`->`, a call), or a fold-expression's expansion
/// applied no operator (spec/ast.md §18.13.6). No entity, no `found`,
/// `how` [`RefLookup::Builtin`]; dumped `(builtin)`. Distinguishes a
/// specialization that reached the token from one that never did (a
/// discarded `if constexpr` branch has no row).
pub const SPEC_BUILTIN: u8 = 32;

/// What one dependent name of a template pattern resolved to in one
/// specialization (spec/ast.md §18.9; 32 bytes).
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
    /// The lookup-context (naming) class ([class.access.base]/5, canonical
    /// first declaration) for a resolved `Member` or `Qualified` lookup in
    /// a class, else `None` (§18.9.1).
    pub naming_class: Option<DeclId>,
    /// [`REF_UNRESOLVED`], [`REF_MEMBER`] (the entry's),
    /// [`REF_REWRITTEN`], [`REF_REVERSED`], [`SPEC_BUILTIN`] (this
    /// specialization's resolution of an operator, §18.13.6).
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

/// The id of a substitution key (spec/ast.md §18.14; index + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
pub struct SubstKeyId(NonZeroU32);

impl SubstKeyId {
    /// The key's index in commit order.
    #[inline]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    fn from_index(i: usize) -> Self {
        let n = u32::try_from(i + 1).expect("substitution key table exhausted");
        SubstKeyId(NonZeroU32::new(n).expect("index + 1 is never 0"))
    }
}

/// The kind of substitution a key names (spec/ast.md §18.14.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
#[repr(u8)]
pub enum SubstKind {
    /// A class or variable template partial specialization matched a
    /// specialization's arguments ([temp.spec.partial.match]).
    PartialMatch = 1,
    /// The associated constraints of a declaration, or a concept-id, were
    /// checked for satisfaction ([temp.constr.constr]).
    Satisfaction = 2,
    /// A default template argument was substituted ([temp.param]/10).
    DefaultArg = 3,
    /// An alias template's type-id was substituted ([temp.alias]/2).
    Alias = 4,
}

impl SubstKind {
    /// Every kind, in discriminant order.
    pub const ALL: [SubstKind; 4] =
        [SubstKind::PartialMatch, SubstKind::Satisfaction, SubstKind::DefaultArg, SubstKind::Alias];

    /// The kind of discriminant `b`.
    pub fn from_u8(b: u8) -> Option<SubstKind> {
        Self::ALL.into_iter().find(|k| *k as u8 == b)
    }

    /// The dump's spelling (§18.14.6).
    pub const fn spelling(self) -> &'static str {
        match self {
            SubstKind::PartialMatch => "partial",
            SubstKind::Satisfaction => "sat",
            SubstKind::DefaultArg => "default",
            SubstKind::Alias => "alias",
        }
    }
}

/// How a keyed substitution completed (spec/ast.md §18.14.1): not part
/// of a key's identity (one identity has one outcome).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default, kiln_snap::Snap)]
#[repr(u8)]
pub enum SubstOutcome {
    /// Matched (`PartialMatch`), satisfied (`Satisfaction`), substituted
    /// (`DefaultArg`, `Alias`).
    #[default]
    Substituted = 1,
    /// A satisfaction check that completed with the answer "not
    /// satisfied" (`Satisfaction` only).
    Unsatisfied = 2,
}

impl SubstOutcome {
    /// The outcome of discriminant `b`.
    pub fn from_u8(b: u8) -> Option<SubstOutcome> {
        match b {
            1 => Some(SubstOutcome::Substituted),
            2 => Some(SubstOutcome::Unsatisfied),
            _ => None,
        }
    }
}

/// One completed substitution of a template pattern with concrete
/// arguments (spec/ast.md §18.14.1; 24 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub struct SubstKey {
    /// What was substituted.
    pub kind: SubstKind,
    /// How it completed.
    pub outcome: SubstOutcome,
    /// The template (first declaration): the partial specialization, the
    /// constrained declaration or concept, the template whose parameter
    /// has the default, the alias template.
    pub template: DeclId,
    /// The canonical arguments (`Types::canonical_args`).
    pub args: ArgListId,
    /// The payload (0 = none): a `PartialMatch`'s deduced argument list
    /// (an [`ArgListId`]'s raw value), a `DefaultArg`'s parameter (a
    /// [`DeclId`]'s raw value, part of the identity).
    pub extra: u32,
    /// The key's rows: start and length in the row table (the length
    /// may be 0).
    rows: (u32, u32),
}

impl SubstKey {
    /// The identity the key is interned by: `extra` counts only for a
    /// `DefaultArg` (its parameter).
    pub fn identity(&self) -> SubstIdentity {
        SubstIdentity::new(self.kind, self.template, self.args, self.extra)
    }

    /// The number of the key's rows.
    #[inline]
    pub fn row_len(&self) -> u32 {
        self.rows.1
    }

    /// A `PartialMatch`'s deduced argument list.
    pub fn deduced(&self) -> Option<ArgListId> {
        if self.kind == SubstKind::PartialMatch { ArgListId::try_new(self.extra) } else { None }
    }

    /// A `DefaultArg`'s template parameter (the one whose default was
    /// substituted).
    pub fn parameter(&self) -> Option<DeclId> {
        if self.kind == SubstKind::DefaultArg { DeclId::try_new(self.extra) } else { None }
    }
}

/// The identity of a substitution key: (kind, template, canonical
/// arguments, the `DefaultArg` parameter or 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubstIdentity(u8, DeclId, ArgListId, u32);

impl SubstIdentity {
    /// The identity of a key of `kind` with these fields (`extra` kept
    /// only for a `DefaultArg`).
    pub fn new(kind: SubstKind, template: DeclId, args: ArgListId, extra: u32) -> Self {
        let extra = if kind == SubstKind::DefaultArg { extra } else { 0 };
        SubstIdentity(kind as u8, template, args, extra)
    }
}

/// What one dependent entry of a key's pattern bound to under the key
/// (spec/ast.md §18.14.2): a [`SpecRef`] without `spec` (28 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub struct SubstRef {
    /// The pattern's entry (in the main table, `DEPENDENT`).
    pub entry: WrittenRefId,
    /// The declaration the name denotes under the key.
    pub entity: Option<DeclId>,
    /// What lookup found, when it differs from `entity`.
    pub found: Option<DeclId>,
    /// The argument-dependent set of a call's lookup (§18.7.4).
    pub adl: Option<AdlSetId>,
    /// The using-directive ordinary lookup found the selected declaration
    /// through at the pattern's definition (§18.7.3).
    pub udir: Option<DeclId>,
    /// The lookup-context (naming) class ([class.access.base]/5, canonical
    /// first declaration) for a resolved `Member` or `Qualified` lookup in
    /// a class, else `None` (§18.9.1, §18.14.2).
    pub naming_class: Option<DeclId>,
    /// [`SpecRef::flags`]'s bits, with [`SUBST_DEPENDENT`] and
    /// [`SUBST_EXPANDED`].
    pub flags: u8,
    /// How lookup found the name (§18.7.2).
    pub how: RefLookup,
}

impl SubstRef {
    /// The row of `r` without its specialization.
    pub fn of_spec(r: &SpecRef) -> SubstRef {
        SubstRef {
            entry: r.entry,
            entity: r.entity,
            found: r.found,
            adl: r.adl,
            udir: r.udir,
            naming_class: r.naming_class,
            flags: r.flags,
            how: r.how,
        }
    }

    /// The `how` the dump leaves out (spec/ast.md §18.7.5), with the
    /// qualifier of the pattern entry `r`.
    pub fn default_how(&self, r: &WrittenRef) -> RefLookup {
        WrittenRef { entity: self.entity, flags: self.flags, ..*r }.default_how()
    }
}

/// [`SubstRef::flags`]: the entry was substituted under the key and is
/// still dependent (its name is resolved by a later substitution); no
/// entity, `how` `None`.
pub const SUBST_DEPENDENT: u8 = 1;

/// [`SubstRef::flags`]: the row's entry is not written in the key's own
/// template. It was reached through an alias template-id that the key's
/// pattern keeps only as its expansion ([temp.alias]/2).
pub const SUBST_EXPANDED: u8 = 64;

/// What one slot of a key's root holds (spec/ast.md §18.14.10).
/// `PartialMatch` keys write `Args` only; the other tags arrive with
/// their kinds' keys, and a later tag may be added (hence
/// `non_exhaustive`).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, kiln_snap::Snap)]
#[repr(u8)]
pub enum RootTag {
    /// The substitution never reached the slot.
    NotReached = 0,
    /// The slot's substitution failed.
    Failed = 1,
    /// An expression ([`ExprId`]).
    Expr = 2,
    /// A type ([`QualType`]).
    Type = 3,
    /// A template argument list ([`ArgListId`], interned, not
    /// canonical).
    Args = 4,
}

impl RootTag {
    /// The tag of discriminant `b`.
    pub fn from_u8(b: u8) -> Option<RootTag> {
        match b {
            0 => Some(RootTag::NotReached),
            1 => Some(RootTag::Failed),
            2 => Some(RootTag::Expr),
            3 => Some(RootTag::Type),
            4 => Some(RootTag::Args),
            _ => None,
        }
    }
}

/// One slot of a key's root (spec/ast.md §18.14.10; 12 bytes): a handle
/// on what the key's substitution built for one place of its pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub struct RootSlot {
    /// What the slot holds.
    pub tag: RootTag,
    /// The handle's raw id (0 for `NotReached` and `Failed`).
    a: u32,
    /// Reserved (0).
    b: u32,
}

impl RootSlot {
    /// A slot the substitution never reached.
    pub const NOT_REACHED: RootSlot = RootSlot { tag: RootTag::NotReached, a: 0, b: 0 };
    /// A slot whose substitution failed.
    pub const FAILED: RootSlot = RootSlot { tag: RootTag::Failed, a: 0, b: 0 };

    /// A slot holding argument list `l`.
    pub fn args(l: ArgListId) -> RootSlot {
        RootSlot { tag: RootTag::Args, a: l.raw(), b: 0 }
    }

    /// A slot holding expression `e`.
    pub fn expr(e: ExprId) -> RootSlot {
        RootSlot { tag: RootTag::Expr, a: e.raw(), b: 0 }
    }

    /// A slot holding type `q`.
    pub fn ty(q: QualType) -> RootSlot {
        RootSlot { tag: RootTag::Type, a: q.raw(), b: 0 }
    }

    /// The argument list of an `Args` slot.
    pub fn as_args(&self) -> Option<ArgListId> {
        (self.tag == RootTag::Args).then(|| ArgListId::try_new(self.a)).flatten()
    }

    /// The expression of an `Expr` slot.
    pub fn as_expr(&self) -> Option<ExprId> {
        (self.tag == RootTag::Expr).then(|| ExprId::try_new(self.a)).flatten()
    }

    /// The type of a `Type` slot.
    pub fn as_type(&self) -> Option<QualType> {
        (self.tag == RootTag::Type).then(|| QualType::from_raw(self.a)).flatten()
    }

    /// The handle's raw id (0 for `NotReached` and `Failed`).
    pub fn raw(&self) -> u32 {
        self.a
    }

    /// The slot with tag `tag` and raw handle `a` (an image's or a
    /// snapshot's), unchecked: the reader validates the handle.
    pub fn from_raw(tag: RootTag, a: u32) -> RootSlot {
        RootSlot { tag, a, b: 0 }
    }

    /// The reserved word (0 in every slot kiln writes).
    pub fn reserved(&self) -> u32 {
        self.b
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
        if let Some(Unrecorded::Prefix { files: f } | Unrecorded::PrefixKeys { files: f }) = self
            .unrecorded
            .iter_mut()
            .rev()
            .find(|u| matches!(u, Unrecorded::Prefix { .. } | Unrecorded::PrefixKeys { .. }))
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

    /// The entries whose `enclosing` is `d` (an entry a declarator group
    /// shares is enclosed by the group's first declaration only; see
    /// [`holders`](Self::holders)).
    pub fn enclosed_by(&self, d: DeclId) -> impl Iterator<Item = (WrittenRefId, &WrittenRef)> + '_ {
        self.iter().filter(move |(_, r)| r.enclosing == Some(d))
    }

    /// Appends `r`; `None` (and nothing recorded) when off. A qualifier
    /// from [`global_link`](Self::global_link) is stored as
    /// [`RefQualifier::GLOBAL`], with its `::` as the entry's lead, one
    /// from [`specifier_link`](Self::specifier_link) as
    /// [`RefQualifier::SPECIFIER`], with the specifier's first token.
    pub fn push(&mut self, mut r: WrittenRef) -> Option<WrittenRefId> {
        if !self.on {
            return None;
        }
        let tok = r.qualifier.colon().and_then(|k| self.colons.get(k).copied());
        let spec = r.qualifier.link() == QualLink::Specifier;
        match r.qualifier.link() {
            QualLink::Global => r.qualifier = RefQualifier::GLOBAL,
            QualLink::Specifier => r.qualifier = RefQualifier::SPECIFIER,
            QualLink::None | QualLink::Entry(_) => {}
        }
        self.entries.push(r);
        let id = WrittenRefId::from_index(self.entries.len() - 1);
        if let Some(c) = tok {
            let (colon, spec) = if spec { (Loc::INVALID, c) } else { (c, Loc::INVALID) };
            let none = Loc::INVALID;
            let l = RefLead {
                entry: id,
                colon,
                tilde: none,
                spec,
                close: none,
                langle: none,
                rangle: none,
            };
            self.leads.push(l);
        }
        Some(id)
    }

    /// The qualifier link of a leading `::` at `colon` (spec/ast.md
    /// §18.11.2): an entry made with it has [`RefQualifier::GLOBAL`] and
    /// [`global_colon`](Self::global_colon) `colon`. Off, plain
    /// [`RefQualifier::GLOBAL`].
    pub fn global_link(&mut self, colon: Loc) -> RefQualifier {
        let k = self.colons.len() as u32;
        if !self.on || k >= SPEC_BIT {
            return RefQualifier::GLOBAL;
        }
        self.colons.push(colon);
        RefQualifier(GLOBAL_BIT | k)
    }

    /// The qualifier link of a `decltype(...)::` or splice specifier
    /// whose first token is `first` (spec/ast.md §18.11.2, WP-2828): an
    /// entry made with it (the component after the specifier) has
    /// [`RefQualifier::SPECIFIER`] and [`specifier_lo`](Self::specifier_lo)
    /// `first`. Off, plain [`RefQualifier::SPECIFIER`].
    pub fn specifier_link(&mut self, first: Loc) -> RefQualifier {
        let k = self.colons.len() as u32;
        if !self.on || k >= SPEC_BIT - 2 {
            return RefQualifier::SPECIFIER;
        }
        self.colons.push(first);
        RefQualifier(GLOBAL_BIT | SPEC_BIT | k)
    }

    /// Notes that entry `id` names a destructor whose `~` is at `tilde`
    /// (its `range` is the class name's, §18.11.2).
    pub fn set_tilde(&mut self, id: WrittenRefId, tilde: Loc) {
        self.set_lead(id, None, Some(tilde), None);
    }

    /// Notes that the `operator` entry `id` (a `[` or `(`) is closed by
    /// the `]` or `)` at `close` (spec/ast.md §18.13).
    pub fn set_close(&mut self, id: WrittenRefId, close: Loc) {
        self.set_lead(id, None, None, Some(close));
    }

    /// Sets the leading `::` (an entry whose qualifier is
    /// [`RefQualifier::GLOBAL`]), the `~` and the closing token of entry
    /// `id`, each when given (a carried table's, §18.10).
    pub fn set_lead(
        &mut self,
        id: WrittenRefId,
        colon: Option<Loc>,
        tilde: Option<Loc>,
        close: Option<Loc>,
    ) {
        let l = self.lead_mut(id);
        if let Some(c) = colon {
            l.colon = c;
        }
        if let Some(t) = tilde {
            l.tilde = t;
        }
        if let Some(c) = close {
            l.close = c;
        }
    }

    /// Sets the specifier's first token of entry `id`, whose qualifier
    /// is [`RefQualifier::SPECIFIER`] (a carried table's, §18.10).
    pub fn set_specifier_lo(&mut self, id: WrittenRefId, first: Loc) {
        self.lead_mut(id).spec = first;
    }

    /// Sets the `<` and `>` of the template argument list after entry
    /// `id`, a friend function template specialization's name.
    pub fn set_args(&mut self, id: WrittenRefId, langle: Loc, rangle: Loc) {
        let l = self.lead_mut(id);
        l.langle = langle;
        l.rangle = rangle;
    }

    /// The row of entry `id`, inserted in order when it has none.
    fn lead_mut(&mut self, id: WrittenRefId) -> &mut RefLead {
        let at = match self.leads.last() {
            Some(l) if l.entry < id => self.leads.len(),
            _ => self.leads.partition_point(|l| l.entry < id),
        };
        if self.leads.get(at).is_none_or(|l| l.entry != id) {
            let none = Loc::INVALID;
            let l = RefLead {
                entry: id,
                colon: none,
                tilde: none,
                spec: none,
                close: none,
                langle: none,
                rangle: none,
            };
            self.leads.insert(at, l);
        }
        &mut self.leads[at]
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

    /// The location of the `]` or `)` that closes the `[` or `(` of the
    /// `operator` entry `id` (a subscript or a call of `operator()` or of
    /// a surrogate call function, spec/ast.md §18.13).
    pub fn close(&self, id: WrittenRefId) -> Option<Loc> {
        self.lead(id).map(|l| l.close).filter(|l| l.is_valid())
    }

    /// The first token of the `decltype(...)::` or splice specifier
    /// before entry `id`, whose qualifier is [`RefQualifier::SPECIFIER`]:
    /// the `decltype` keyword, the `[:` or the `template` of
    /// `template [: r :]<...>::` (`None` for any other entry, and for
    /// one whose token was not recorded).
    pub fn specifier_lo(&self, id: WrittenRefId) -> Option<Loc> {
        self.lead(id).map(|l| l.spec).filter(|l| l.is_valid())
    }

    /// The `<` and `>` of the template argument list written after entry
    /// `id`, a friend function template specialization's name (`None` for
    /// any other entry, §18.11.2).
    pub fn template_args(&self, id: WrittenRefId) -> Option<(Loc, Loc)> {
        let l = self.lead(id)?;
        (l.langle.is_valid() && l.rangle.is_valid()).then_some((l.langle, l.rangle))
    }

    /// Where the name of entry `id` begins as an expression stores it:
    /// the `~` of a destructor name, else `range.lo`. The location of a
    /// `DeclRefExpr` or `MemberExpr` built from an id-expression or a
    /// member name equals it (the join, spec/ast.md §18.11.1).
    pub fn name_lo(&self, id: WrittenRefId) -> Loc {
        self.tilde(id).unwrap_or(self.get(id).range.lo)
    }

    /// Where the qualified name ending in entry `id` begins: the leading
    /// `::`, the first token of a `decltype(...)::` or splice specifier
    /// ([`specifier_lo`](Self::specifier_lo)), else the first component's
    /// `range.lo` (the start of `id`'s own name, [`name_lo`](Self::name_lo),
    /// when it is unqualified).
    pub fn qualified_lo(&self, id: WrittenRefId) -> Loc {
        let mut cur = id;
        // Each qualifier precedes its entry: the walk ends.
        while let QualLink::Entry(q) = self.get(cur).qualifier.link() {
            cur = q;
        }
        match self.get(cur).qualifier.link() {
            QualLink::Global => self.global_colon(cur).unwrap_or(self.get(cur).range.lo),
            QualLink::Specifier => self.specifier_lo(cur).unwrap_or(self.get(cur).range.lo),
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
        if flags & REF_DEPENDENT == 0 {
            self.drop_found(id);
        }
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
        // A callee recorded before its arguments, resolved at the
        // definition: no longer dependent, so no found set (§18.12).
        self.drop_found(id);
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

    /// The node `n` noted for entry `id` resolved after all (a rebuilt
    /// overload set whose call turned out not to be dependent): it no
    /// longer stands for the entry (§18.9.4).
    pub fn forget_pattern_node(&mut self, n: PatternNode, id: WrittenRefId) {
        if self.spec_nodes.get(&n) == Some(&id) {
            self.spec_nodes.remove(&n);
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

    // The found set of a dependent entry (spec/ast.md §18.12).

    /// The interned id of the found set `decls` with `how` and `flags`
    /// (one copy per distinct set).
    pub fn intern_found(&mut self, decls: &[FoundDecl], how: RefLookup, flags: u8) -> FoundSetId {
        let key: FoundKey = (how, flags, decls.into());
        if let Some(&s) = self.found_index.get(&key) {
            return s;
        }
        let s = FoundSetId::from_index(self.found_sets.len());
        let start = u32::try_from(self.found_decls.len()).expect("found-set elements exhausted");
        self.found_decls.extend_from_slice(decls);
        let len = decls.len() as u32;
        self.found_sets.push(FoundSetRec { start, len, how, flags });
        self.found_index.insert(key, s);
        s
    }

    /// Gives the dependent entry `id` the found set `s` (replacing one it
    /// has); nothing when off, or while pattern nodes are not remembered
    /// (§18.8).
    pub fn set_found(&mut self, id: WrittenRefId, s: FoundSetId) {
        if !self.on || self.no_nodes {
            return;
        }
        let at = match self.found_rows.last() {
            Some(&(last, _)) if last < id => self.found_rows.len(),
            None => 0,
            _ => self.found_rows.partition_point(|&(e, _)| e < id),
        };
        match self.found_rows.get_mut(at) {
            Some(r) if r.0 == id => r.1 = s,
            _ => self.found_rows.insert(at, (id, s)),
        }
    }

    /// Removes the found set of entry `id`, which is no longer dependent
    /// (one branch when no entry has one; the newest row is the usual
    /// one: calls nest).
    #[inline]
    pub fn drop_found(&mut self, id: WrittenRefId) {
        if self.found_rows.is_empty() {
            return;
        }
        if self.found_rows.last().is_some_and(|&(e, _)| e == id) {
            self.found_rows.pop();
        } else if let Ok(i) = self.found_rows.binary_search_by_key(&id, |&(e, _)| e) {
            self.found_rows.remove(i);
        }
    }

    /// The interned id of the found set of entry `id` (equal ids are
    /// equal sets); `None` when it is not `DEPENDENT` or no lookup ran at
    /// its definition.
    pub fn found_set_id(&self, id: WrittenRefId) -> Option<FoundSetId> {
        if self.found_rows.is_empty() {
            return None;
        }
        let i = self.found_rows.binary_search_by_key(&id, |&(e, _)| e).ok()?;
        (self.entries.get(id.index())?.flags & REF_DEPENDENT != 0).then_some(self.found_rows[i].1)
    }

    /// The found set of entry `id` (§18.12): what the name's lookup at
    /// the template definition found. `None` when the entry is not
    /// `DEPENDENT`, or no lookup ran at its definition (a member of an
    /// unknown specialization, a dependent qualifier, a dependent type);
    /// `Some` with no element when lookup ran and found nothing.
    pub fn found_at_definition(&self, id: WrittenRefId) -> Option<FoundSet<'_>> {
        self.found_set_id(id).map(|s| self.found_set(s))
    }

    /// The interned found set `s`.
    pub fn found_set(&self, s: FoundSetId) -> FoundSet<'_> {
        let r = self.found_sets[s.index()];
        let lo = r.start as usize;
        FoundSet { decls: &self.found_decls[lo..lo + r.len as usize], how: r.how, flags: r.flags }
    }

    /// The number of distinct found sets.
    pub fn found_set_count(&self) -> usize {
        self.found_sets.len()
    }

    /// Every entry with a found set and its set's id, by entry (rows of
    /// entries no longer `DEPENDENT` included; [`found_set_id`](Self::found_set_id)
    /// leaves those out).
    pub fn found_rows(&self) -> &[(WrittenRefId, FoundSetId)] {
        &self.found_rows
    }

    /// The using-directive through which the definition lookup of
    /// dependent entry `id` found `decl` (an element's `decl`, §18.9.1).
    #[inline]
    pub fn found_udir(&self, id: WrittenRefId, decl: DeclId) -> Option<DeclId> {
        self.pattern_udir(id, decl)
    }

    /// The dependent entries whose found set has `d` as an element's
    /// `decl` (a first declaration) or `found` (a using-shadow), in entry
    /// order: with the entries naming `d` and the pattern entries of
    /// [`specs_naming`](Self::specs_naming)`(d)`, what a rename of `d`
    /// edits (§18.12).
    pub fn entries_finding(&self, d: DeclId) -> impl Iterator<Item = WrittenRefId> + '_ {
        let hit: Vec<bool> = (0..self.found_sets.len())
            .map(|i| self.found_set(FoundSetId::from_index(i)).contains(d))
            .collect();
        self.found_rows
            .iter()
            .filter(move |&&(id, s)| {
                hit[s.index()] && self.entries[id.index()].flags & REF_DEPENDENT != 0
            })
            .map(|&(id, _)| id)
    }

    /// The bytes (live, reserved) of the found sets (§18.12): the rows,
    /// the sets, their elements and the content index (a hash map slot
    /// counted twice its pair, each key's list once more).
    pub fn found_mem(&self) -> (u64, u64) {
        let row = size_of::<(WrittenRefId, FoundSetId)>() as u64;
        let set = size_of::<FoundSetRec>() as u64;
        let el = size_of::<FoundDecl>() as u64;
        let key = (size_of::<(FoundKey, FoundSetId)>() as u64) * 2;
        let lists = self.found_decls.len() as u64 * el;
        let live = self.found_rows.len() as u64 * row
            + self.found_sets.len() as u64 * set
            + lists
            + self.found_index.len() as u64 * key
            + lists;
        let reserved = self.found_rows.capacity() as u64 * row
            + self.found_sets.capacity() as u64 * set
            + self.found_decls.capacity() as u64 * el
            + self.found_index.capacity() as u64 * key
            + lists;
        (live, reserved)
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
        !self.spec_pending.is_empty() || !self.subst.pending.is_empty()
    }

    /// The pattern entry and specialization of the rebuilt overload set
    /// `x` while it awaits resolution.
    pub fn spec_pending_for(&self, x: ExprId) -> Option<(WrittenRefId, DeclId)> {
        self.spec_pending.get(&x).copied()
    }

    /// Whether the rebuilt overload set `x` awaits resolution, for a
    /// specialization's row, a key scope's row (§18.14.3) or both.
    pub fn is_spec_pending(&self, x: ExprId) -> bool {
        self.spec_pending.contains_key(&x) || self.subst.pending.contains_key(&x)
    }

    /// Takes the pending resolution of `x` (it is being resolved).
    pub fn take_spec_pending(&mut self, x: ExprId) -> Option<(WrittenRefId, DeclId)> {
        if self.spec_pending.is_empty() { None } else { self.spec_pending.remove(&x) }
    }

    /// The rebuilt overload set `x` of pattern entry `entry` awaits
    /// overload resolution inside key scope `scope` (§18.14.3).
    pub fn defer_subst(&mut self, x: ExprId, entry: WrittenRefId, scope: ScopeRef) {
        self.subst.pending.insert(x, (entry, scope));
    }

    /// Takes the keyed half of the pending resolution of `x`.
    pub fn take_subst_pending(&mut self, x: ExprId) -> Option<(WrittenRefId, ScopeRef)> {
        if self.subst.pending.is_empty() { None } else { self.subst.pending.remove(&x) }
    }

    /// Remembers that resolving the overload set `x` made the keyed row
    /// `index` of scope `scope`.
    pub fn set_subst_last(&mut self, x: ExprId, scope: ScopeRef, index: u32) {
        self.subst.last = Some((x, scope, index));
    }

    /// The keyed row resolving the overload set `x` made, once.
    pub fn take_subst_last(&mut self, x: ExprId) -> Option<(ScopeRef, u32)> {
        match self.subst.last.take() {
            Some((e, s, i)) if e == x => Some((s, i)),
            _ => None,
        }
    }

    /// The rebuilt overload set `from` goes on as `to`.
    pub fn retarget_spec(&mut self, from: ExprId, to: ExprId) {
        if let Some(p) = self.spec_pending.remove(&from) {
            self.spec_pending.insert(to, p);
        }
        if let Some(p) = self.subst.pending.remove(&from) {
            self.subst.pending.insert(to, p);
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

    /// Remembers the lookup-context (naming) class `naming` of the
    /// non-overloaded member or qualified-member expression `e` just built.
    /// Overwriting an unconsumed entry is expected because non-dependent and
    /// sema-synthesized accesses have no `PatternNode::Expr`.
    #[inline]
    pub fn set_last_naming(&mut self, e: ExprId, naming: DeclId) {
        self.last_naming = Some((e, naming));
    }

    /// Consumes the remembered naming class if it belongs to `e` or
    /// `peeled`, clearing the slot unconditionally so a stale value never
    /// attaches to a later expression.
    #[inline]
    pub fn take_last_naming(&mut self, e: ExprId, peeled: ExprId) -> Option<DeclId> {
        self.last_naming.take().and_then(|(id, naming)| (id == e || id == peeled).then_some(naming))
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

    /// Moves the live entries recorded since `mark` under specialization
    /// `from` to specialization `to` (a function template's default
    /// template argument, substituted before the specialization it is
    /// part of was formed, §18.9.3). One that `to` already has is
    /// dropped (one resolution is kept once, §18.9.1).
    pub fn rekey_specs_since(&mut self, mark: u32, from: DeclId, to: DeclId) {
        for i in mark as usize..self.specs.len() {
            let r = self.specs[i];
            if r.flags & SPEC_DEAD == 0 && r.spec == from {
                let _ = self.update_spec(SpecRefId::from_index(i), |r| r.spec = to);
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
        self.truncate_subst(m);
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
        let k = self.found_rows.partition_point(|&(id, _)| id.index() < n);
        self.found_rows.truncate(k);
        self.spec_pending.retain(|_, (id, _)| id.index() < n);
        self.subst.pending.retain(|_, (id, _)| id.index() < n);
        for r in &mut self.specs {
            if r.entry.index() >= n {
                r.flags |= SPEC_DEAD;
            }
        }
        self.spec_seen.retain(|r, _| r.entry.index() < n);
        // A group whose first declaration the dropped text made.
        if !self.groups.is_empty() {
            let decls = &self.group_decls;
            self.groups.retain(|g| decls[g.start as usize].index() <= m.decls);
            let end = self.groups.iter().map(|g| (g.start + g.len) as usize).max().unwrap_or(0);
            self.group_decls.truncate(end);
        }
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

    /// The declarator group whose first declaration is `first`, in
    /// declaration order, when an entry is shared by it (spec/ast.md
    /// §18.3.1).
    pub fn group(&self, first: DeclId) -> Option<&[DeclId]> {
        let i = self.group_at(first).ok()?;
        Some(self.group_run(self.groups[i]))
    }

    /// The declarations that hold entry `id`: the declarator group of its
    /// `enclosing` for an entry with [`REF_GROUP`], else its `enclosing`
    /// alone (empty when it has none; spec/ast.md §18.3.1).
    pub fn holders(&self, id: WrittenRefId) -> &[DeclId] {
        let Some(r) = self.entries.get(id.index()) else { return &[] };
        match r.enclosing {
            Some(d) if r.flags & REF_GROUP != 0 => {
                self.group(d).unwrap_or_else(|| self.group_decls_one(id))
            }
            _ => self.group_decls_one(id),
        }
    }

    fn group_decls_one(&self, id: WrittenRefId) -> &[DeclId] {
        self.entries.get(id.index()).map_or(&[], |r| r.enclosing.as_slice())
    }

    /// Every declarator group with a shared entry, by first declaration.
    pub fn groups(&self) -> impl Iterator<Item = &[DeclId]> + '_ {
        self.groups.iter().map(|&g| self.group_run(g))
    }

    fn group_run(&self, g: DeclGroupRec) -> &[DeclId] {
        let lo = g.start as usize;
        self.group_decls.get(lo..lo + g.len as usize).unwrap_or(&[])
    }

    fn group_at(&self, first: DeclId) -> Result<usize, usize> {
        self.groups.binary_search_by_key(&first, |&g| self.group_decls[g.start as usize])
    }

    /// The declarator group `decls` (in declaration order) shares the
    /// decl-specifier-seq whose names are the entries `specs` (indices,
    /// half-open): those its first declaration holds get [`REF_GROUP`],
    /// and the group its row when one does (spec/ast.md §18.3.1). A group
    /// of one has neither.
    pub fn share_group(&mut self, specs: (u32, u32), decls: &[DeclId]) {
        let &[first, _, ..] = decls else { return };
        if !self.on {
            return;
        }
        let hi = (specs.1 as usize).min(self.entries.len());
        let lo = (specs.0 as usize).min(hi);
        let mut any = false;
        for r in &mut self.entries[lo..hi] {
            if r.enclosing == Some(first) {
                r.flags |= REF_GROUP;
                any = true;
            }
        }
        if !any {
            return;
        }
        if let Err(at) = self.group_at(first) {
            self.push_group_at(at, decls);
        }
    }

    /// Adds the group `decls` (its first declaration not yet a group's):
    /// a module image's group, after its entries (spec/ast.md §18.10.2).
    pub fn push_group(&mut self, decls: &[DeclId]) -> bool {
        let &[first, _, ..] = decls else { return false };
        let Err(at) = self.group_at(first) else { return false };
        self.push_group_at(at, decls);
        true
    }

    fn push_group_at(&mut self, at: usize, decls: &[DeclId]) {
        let start = self.group_decls.len() as u32;
        self.group_decls.extend_from_slice(decls);
        self.groups.insert(at, DeclGroupRec { start, len: decls.len() as u32 });
    }

    /// Whether an entry after `m` is still enclosed by a declaration that
    /// existed at `m` (a claim would change it).
    pub fn has_unclaimed(&self, m: RefMark) -> bool {
        self.entries
            .iter()
            .skip(m.entries as usize)
            .any(|r| r.enclosing.is_none_or(|e| e.index() <= m.decls))
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
        let (found_live, found_reserved) = self.found_mem();
        let grow = size_of::<DeclGroupRec>() as u64;
        let gdecl = size_of::<DeclId>() as u64;
        Some(MemRow::new(
            "written_refs",
            self.entries.len() as u64,
            self.entries.len() as u64 * sz
                + self.pending.len() as u64 * pend
                + self.designators.len() as u64 * desig
                + self.leads.len() as u64 * lead
                + self.colons.len() as u64 * colon
                + prov_live
                + spec_live
                + found_live
                + self.groups.len() as u64 * grow
                + self.group_decls.len() as u64 * gdecl,
            self.entries.capacity() as u64 * sz
                + self.pending.capacity() as u64 * pend
                + self.designators.capacity() as u64 * desig
                + self.leads.capacity() as u64 * lead
                + self.colons.capacity() as u64 * colon
                + prov_reserved
                + spec_reserved
                + found_reserved
                + self.groups.capacity() as u64 * grow
                + self.group_decls.capacity() as u64 * gdecl,
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
                Unrecorded::PrefixKeys { files } => {
                    let _ = writeln!(out, "unrecorded  precompiled prefix keys ({files} files)");
                }
                Unrecorded::ModuleKeys { module, files } => {
                    let main = files.first().map_or("?", |&f| {
                        let n = sm.file_name(f);
                        n.rsplit(['/', '\\']).next().unwrap_or(n)
                    });
                    let _ = writeln!(
                        out,
                        "unrecorded  module {module} keys ({main}, {} files)",
                        files.len()
                    );
                }
                Unrecorded::ModuleKeysLeftOut { module, keys, rows } => {
                    let _ = writeln!(
                        out,
                        "unrecorded  module {module} keys left out ({keys} keys, {rows} rows)"
                    );
                }
            }
        }
    }

    /// The dump's `q=`, `tilde=` and `close=` fields of entry `id` (§18.6).
    /// ` in=` and the entry's `enclosing` (`(tu)` for the translation
    /// unit `tu`), then ` shared=` and the rest of the declarator group
    /// that holds a shared entry (§18.3.1).
    fn dump_holders(
        &self,
        id: WrittenRefId,
        tu: DeclId,
        decl: &dyn Fn(DeclId, &mut String),
        out: &mut String,
    ) {
        let r = self.get(id);
        match r.enclosing {
            Some(d) if d == tu => out.push_str(" in=(tu)"),
            Some(d) => {
                out.push_str(" in=");
                decl(d, out);
            }
            None => {}
        }
        if r.flags & REF_GROUP == 0 {
            return;
        }
        out.push_str(" shared=");
        for (k, &g) in self.holders(id).iter().skip(1).enumerate() {
            if k > 0 {
                out.push(',');
            }
            decl(g, out);
        }
    }

    fn dump_lead(&self, id: WrittenRefId, out: &mut String, pos: &dyn Fn(Loc) -> String) {
        match self.get(id).qualifier.link() {
            QualLink::None => {}
            QualLink::Global => {
                out.push_str(" q=::");
                if let Some(c) = self.global_colon(id) {
                    out.push_str(&pos(c));
                }
            }
            QualLink::Specifier => {
                out.push_str(" q=specifier");
                if let Some(c) = self.specifier_lo(id) {
                    out.push(':');
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
        if let Some(c) = self.close(id) {
            out.push_str(" close=");
            out.push_str(&pos(c));
        }
        if let Some((l, r)) = self.template_args(id) {
            out.push_str(" args=");
            out.push_str(&pos(l));
            out.push('-');
            out.push_str(&pos(r));
        }
    }

    /// The dump (spec/ast.md §18.6): one line per entry.
    #[allow(clippy::too_many_lines, reason = "one block per kind of line, in the dump's order")]
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
            // A `static_assert` has no name: its keyword's location names
            // it (§18.3).
            if ast.decls.get(d).kind() == crate::decl::DeclKind::StaticAssert {
                out.push_str("static_assert@");
                out.push_str(&pos(ast.decls.get(d).loc()));
                return;
            }
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
        let keyed = self.keyed_by_entry();
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
                None if r.flags & REF_BUILTIN != 0 => out.push_str("(builtin)"),
                None => out.push_str("(unresolved)"),
            }
            if let Some(f) = r.found {
                out.push_str(" found=");
                decl(f, &mut out);
            }
            self.dump_lead(id, &mut out, &pos);
            self.dump_holders(id, ast.tu, &decl, &mut out);
            if r.entity.is_some() && r.flags & REF_DEPENDENT != 0 {
                out.push_str(" dep");
            }
            dump_rewrite_flags(r.flags, &mut out);
            if sm.is_macro_loc(r.range.lo) {
                out.push_str(" exp=");
                out.push_str(&pos(sm.expansion_loc(r.range.lo)));
            }
            // The lookup provenance (§18.7.5).
            self.dump_provenance(id, ast, &names, &pos, &decl, &mut out);
            // The found set (§18.12): its size and flags, then one line
            // per element.
            let found = self.found_at_definition(id);
            if let Some(set) = found {
                dump_found_suffix(&set, &mut out);
            }
            out.push('\n');
            if let Some(set) = found {
                let head = format!("{lo}-{hi}  {}  '{text}'", r.role.spelling());
                self.dump_found(id, &set, &head, ast, sm, main, &names, &pos, &decl, &mut out);
            }
            // Its per-specialization lines, sorted: the order of
            // instantiation does not show (§18.9.5).
            if let Some(v) = by_entry.get(&id) {
                let head = format!("{lo}-{hi}  {}  '{text}' -> ", r.role.spelling());
                self.dump_specs(r, v, &head, ast, &canon, &pos, &canon_decl, &mut out);
            }
            // Its keyed rows (§18.14.6), sorted.
            if let Some(v) = keyed.get(&id) {
                let head = format!("{lo}-{hi}  {}  '{text}' -> ", r.role.spelling());
                self.dump_keyed(r, v, &head, ast, &canon, &pos, &canon_decl, &mut out);
            }
        }
        self.dump_key_blocks(ast, &canon, &names, &pos, &mut out);
        out
    }

    /// The element lines of entry `id`'s found set, each starting with
    /// `head` (the entry's positions, role and spelling), sorted by the
    /// position of the element's `decl` (§18.12).
    #[allow(clippy::too_many_arguments, reason = "the dump's shared printers")]
    fn dump_found(
        &self,
        id: WrittenRefId,
        set: &FoundSet<'_>,
        head: &str,
        ast: &crate::Ast,
        sm: &SourceManager,
        main: Option<FileId>,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        decl: &dyn Fn(DeclId, &mut String),
        out: &mut String,
    ) {
        // (another file's name, line, column): the main file first.
        let key = |d: DeclId| match sm.line_col(sm.spelling_loc(ast.decls.get(d).loc())) {
            Some(lc) if Some(lc.file) == main => (String::new(), lc.line, lc.col),
            Some(lc) => {
                let n = sm.file_name(lc.file);
                (n.rsplit(['/', '\\']).next().unwrap_or(n).to_owned(), lc.line, lc.col)
            }
            None => ("?".to_owned(), 0, 0),
        };
        let mut lines: Vec<((String, u32, u32), String)> = set
            .decls
            .iter()
            .map(|e| {
                let mut l = head.to_owned();
                l.push_str(" def=");
                decl(e.decl, &mut l);
                if e.found != e.decl {
                    l.push_str(" found=");
                    decl(e.found, &mut l);
                }
                if let Some(u) = self.found_udir(id, e.decl) {
                    dump_udir(u, ast, names, pos, &mut l);
                }
                l.push('\n');
                (key(e.decl), l)
            })
            .collect();
        lines.sort();
        for (_, l) in lines {
            out.push_str(&l);
        }
    }

    /// A declaration formatted with its template arguments (for a
    /// non-function specialization such as `Table<Cfg>` or
    /// `Der<n::S<int>>`) and its source position, used for `spec=` and
    /// `naming=`.
    pub(super) fn write_spec_decl(
        d: DeclId,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        out: &mut String,
    ) {
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
    }

    /// Prints ` naming=<class>` when `naming_class` is present and differs
    /// from the semantic parent of `entity` (spec/ast.md §18.9.5).
    pub(super) fn dump_naming_class(
        entity: Option<DeclId>,
        naming_class: Option<DeclId>,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        out: &mut String,
    ) {
        if let Some(nc) = naming_class
            && entity.and_then(|e| ast.decls.get(e).ctx()).map(|c| ast.first(c)) != Some(nc)
        {
            out.push_str(" naming=");
            Self::write_spec_decl(nc, ast, names, pos, out);
        }
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
        let mut lines: Vec<String> = rows
            .iter()
            .map(|sr| {
                let mut l = head.to_owned();
                match sr.entity {
                    Some(d) => decl(d, &mut l),
                    None if sr.flags & SPEC_BUILTIN != 0 => l.push_str("(builtin)"),
                    None => l.push_str("(unresolved)"),
                }
                if let Some(f) = sr.found {
                    l.push_str(" found=");
                    decl(f, &mut l);
                }
                l.push_str(" spec=");
                Self::write_spec_decl(sr.spec, ast, names, pos, &mut l);
                Self::dump_naming_class(sr.entity, sr.naming_class, ast, names, pos, &mut l);
                dump_rewrite_flags(sr.flags, &mut l);
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

impl crate::Ast {
    /// The declaration of a dependent entry's found set that the
    /// per-specialization row `r` selected (spec/ast.md §18.12, the join
    /// of the two): the first declaration of what lookup found (`found`,
    /// else `entity`, a using-shadow looked through), a member of a class
    /// template specialization mapped to the pattern's member it was
    /// instantiated from, a variable template's specialization to its
    /// template. `None` for a row without an entity.
    pub fn spec_origin(&self, r: &SpecRef) -> Option<DeclId> {
        let mut d = self.first(self.strip_shadow(r.found.or(r.entity)?));
        // A member of a member class is instantiated from the member of
        // the pattern's member class: one step per enclosing
        // instantiation. A variable template's specialization (a row
        // with no `found`) and a template's pattern step to the
        // template, which lookup found.
        for _ in 0..8 {
            let up = self.instantiated_from(d).or_else(|| {
                self.spec_info(d)
                    .map(|(_, i)| i.from)
                    .filter(|&f| self.decls.get(f).kind() == crate::decl::DeclKind::VarTemplate)
                    .or_else(|| self.described_template(d))
            });
            match up.map(|p| self.first(p)) {
                Some(p) if p != d => d = p,
                _ => break,
            }
        }
        Some(d)
    }
}

/// The ` def=<n>[+adl][+base][+using]` suffix of a dependent entry's
/// dump line (spec/ast.md §18.12.5).
/// The dump's `rewritten` and `reversed` words of an entry's or a
/// per-specialization row's `flags` (spec/ast.md §18.13.7).
fn dump_rewrite_flags(flags: u8, out: &mut String) {
    if flags & REF_REWRITTEN != 0 {
        out.push_str(" rewritten");
    }
    if flags & REF_REVERSED != 0 {
        out.push_str(" reversed");
    }
}

fn dump_found_suffix(set: &FoundSet<'_>, out: &mut String) {
    let _ = write!(out, " def={}", set.decls.len());
    let flags =
        [(FOUND_ADL, "+adl"), (FOUND_DEPENDENT_BASE, "+base"), (FOUND_UNRESOLVED_USING, "+using")];
    for (f, s) in flags {
        if set.flags & f != 0 {
            out.push_str(s);
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
        t.set_lead(ec, None, Some(Loc::from_raw(19)), None);
        let entries: Vec<_> = t.leads().iter().map(|l| l.entry).collect();
        assert_eq!(entries, [ea, ec, dtor]);
        assert_eq!(t.tilde(ec), Some(Loc::from_raw(19)));
        t.truncate(mark);
        assert_eq!(t.leads().len(), 2);
        assert_eq!(t.tilde(dtor), None);
        assert_eq!(size_of::<WrittenRef>(), 28);
    }

    /// Stage 6 (spec/ast.md §18.13, WP-2820): the two roles round trip by
    /// byte, the closing token of a `[`/`(` entry is kept beside it (the
    /// lead row grows to 20 bytes, the entry stays 28) and the rewritten
    /// and reversed flags print on the entry's line.
    #[test]
    fn operator_entries_keep_their_close_and_flags() {
        for r in RefRole::ALL {
            assert_eq!(RefRole::from_u8(r as u8), Some(r));
        }
        assert_eq!(RefRole::Operator.spelling(), "operator");
        assert_eq!(RefRole::UdSuffix.spelling(), "ud-suffix");
        assert_eq!(RefRole::Label.spelling(), "label");
        assert_eq!(RefRole::from_u8(RefRole::ALL.len() as u8), None);
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let sub = t.push(WrittenRef { role: RefRole::Operator, ..at(5, RefQualifier::NONE) });
        let sub = sub.unwrap();
        t.set_close(sub, Loc::from_raw(9));
        assert_eq!(t.close(sub), Some(Loc::from_raw(9)));
        assert_eq!(t.tilde(sub), None);
        assert_eq!(t.global_colon(sub), None);
        // The join location of an operator entry is its own token.
        assert_eq!(t.name_lo(sub), Loc::from_raw(5));
        let mut out = String::new();
        dump_rewrite_flags(REF_REWRITTEN | REF_REVERSED, &mut out);
        assert_eq!(out, " rewritten reversed");
        assert_eq!(size_of::<RefLead>(), 28);
        assert_eq!(size_of::<WrittenRef>(), 28);
    }

    /// WP-2828 (spec/ast.md §18.11.2): `decltype(s)::N::k`, the entry
    /// after the specifier is qualified by it and keeps its first token.
    #[test]
    fn a_specifier_qualifier_keeps_its_first_token() {
        let mut t = WrittenRefs::default();
        // Off: the plain specifier link, nothing kept.
        assert_eq!(t.specifier_link(Loc::from_raw(3)), RefQualifier::SPECIFIER);
        assert!(t.colons.is_empty());
        t.set_on(true);
        let link = t.specifier_link(Loc::from_raw(10));
        assert_eq!(link.link(), QualLink::Specifier);
        // A `::` link handed out after it keeps its own kind.
        assert_eq!(t.global_link(Loc::from_raw(40)).link(), QualLink::Global);
        let en = t.push(at(22, link)).unwrap();
        let ek = t.push(at(25, RefQualifier::entry(en))).unwrap();
        assert_eq!(t.get(en).qualifier, RefQualifier::SPECIFIER);
        let named = WrittenRef { entity: Some(DeclId::new(1)), ..*t.get(en) };
        assert_eq!(named.default_how(), RefLookup::Qualified);
        assert_eq!(t.specifier_lo(en), Some(Loc::from_raw(10)));
        assert_eq!(t.global_colon(en), None);
        assert_eq!(t.specifier_lo(ek), None);
        assert_eq!(t.qualified_lo(ek), Loc::from_raw(10));
        assert_eq!(t.qualified_lo(en), Loc::from_raw(10));
        assert_eq!(t.name_lo(en), Loc::from_raw(22));
        // A SPECIFIER link of no recorded token (a carried table's), then
        // the token set from the carriage.
        let ec = t.push(at(30, RefQualifier::SPECIFIER)).unwrap();
        assert_eq!(t.qualified_lo(ec), Loc::from_raw(30));
        t.set_specifier_lo(ec, Loc::from_raw(28));
        assert_eq!(t.qualified_lo(ec), Loc::from_raw(28));
        let entries: Vec<_> = t.leads().iter().map(|l| l.entry).collect();
        assert_eq!(entries, [en, ec]);
        assert_eq!(RefQualifier::GLOBAL.link(), QualLink::Global);
        assert_eq!(RefQualifier::SPECIFIER.link(), QualLink::Specifier);
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

    /// spec/ast.md §18.3.1 (WP-3144): the entries of a shared
    /// decl-specifier-seq that the first declaration holds get
    /// `REF_GROUP` and every declaration of the group holds them; a name
    /// nested in the specifier, a declarator's own name and a group of one
    /// are unchanged; a truncation drops a group made after its mark.
    #[test]
    fn a_declarator_group_shares_its_specifier() {
        let (ctx, first, second, third, lam) =
            (DeclId::new(1), DeclId::new(3), DeclId::new(4), DeclId::new(5), DeclId::new(2));
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let shared = t.push(entry(Some(first))).unwrap();
        let nested = t.push(entry(Some(lam))).unwrap();
        let own = t.push(entry(Some(second))).unwrap();
        t.share_group((0, 2), &[first]);
        assert_eq!(t.get(shared).flags, 0, "a group of one");
        assert!(t.groups().next().is_none());
        t.share_group((0, 2), &[first, second, third]);
        assert_eq!(t.get(shared).flags, REF_GROUP);
        assert_eq!(t.get(nested).flags, 0);
        assert_eq!(t.get(own).flags, 0);
        assert_eq!(t.holders(shared), &[first, second, third]);
        assert_eq!(t.holders(nested), &[lam]);
        assert_eq!(t.group(first), Some(&[first, second, third][..]));
        assert_eq!(t.group(second), None);
        // Declarations 1..=5 exist at the mark; 6 is made after it.
        let m = t.mark(5);
        let _ = t.push(entry(Some(ctx))).unwrap();
        let inner = t.push(entry(Some(DeclId::new(6)))).unwrap();
        t.share_group((inner.index() as u32, inner.index() as u32 + 1), &[DeclId::new(6), third]);
        assert_eq!(t.groups().count(), 2);
        // Sorted by first declaration.
        assert_eq!(t.groups().map(|g| g[0]).collect::<Vec<_>>(), [first, DeclId::new(6)]);
        t.truncate(m);
        assert_eq!(t.groups().count(), 1);
        assert_eq!(t.holders(shared), &[first, second, third]);
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
            naming_class: None,
            flags: 0,
            how: RefLookup::Member,
        }
    }

    #[test]
    fn spec_row_is_32_bytes() {
        assert_eq!(size_of::<SpecRef>(), 32);
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
    fn forgotten_pattern_node_is_only_its_entrys() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let w = t.push(entry(None)).expect("on");
        let v = t.push(entry(None)).expect("on");
        let x = PatternNode::Expr(ExprId::new(7));
        t.note_pattern_node(x, w);
        // Another entry's forgetting leaves the node alone.
        t.forget_pattern_node(x, v);
        assert_eq!(t.pattern_entry(x), Some(w));
        t.forget_pattern_node(x, w);
        assert_eq!(t.pattern_entry(x), None);
        assert!(!t.has_pattern_nodes());
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
        // Distinct naming classes stay distinct rows (spec/ast.md §18.9.1).
        let n1 =
            t.push_spec(SpecRef { naming_class: Some(DeclId::new(30)), ..spec_row(w, 10, 20) });
        let n2 =
            t.push_spec(SpecRef { naming_class: Some(DeclId::new(31)), ..spec_row(w, 10, 20) });
        assert_ne!(a, n1);
        assert_ne!(n1, n2);
        assert_eq!(
            t.push_spec(SpecRef { naming_class: Some(DeclId::new(30)), ..spec_row(w, 10, 20) }),
            n1
        );
        assert_eq!(t.spec_len(), 3);
    }

    /// spec/ast.md §18.13.6: an operator token is its own pattern node,
    /// apart from the expression's (a dependent `t->m` is the node of both
    /// `->` and `m`), one per slot of a binary fold; a built-in row names
    /// nothing and is kept once, apart from an unresolved one.
    #[test]
    fn operator_nodes_and_builtin_rows() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let m = t.push(dependent()).unwrap();
        let arrow = t.push(WrittenRef { role: RefRole::Operator, ..dependent() }).unwrap();
        let second = t.push(WrittenRef { role: RefRole::Operator, ..dependent() }).unwrap();
        let x = ExprId::new(7);
        t.note_pattern_node(PatternNode::Expr(x), m);
        t.note_pattern_node(PatternNode::Oper(x, 0), arrow);
        t.note_pattern_node(PatternNode::Oper(x, 1), second);
        assert_eq!(t.pattern_entry(PatternNode::Expr(x)), Some(m));
        assert_eq!(t.pattern_entry(PatternNode::Oper(x, 0)), Some(arrow));
        assert_eq!(t.pattern_entry(PatternNode::Oper(x, 1)), Some(second));
        let builtin = SpecRef {
            entity: None,
            flags: SPEC_BUILTIN,
            how: RefLookup::Builtin,
            ..spec_row(arrow, 10, 1)
        };
        let unresolved = SpecRef { flags: REF_UNRESOLVED, how: RefLookup::Unqualified, ..builtin };
        let a = t.push_spec(builtin);
        assert_eq!(t.push_spec(builtin), a);
        assert_ne!(t.push_spec(unresolved), a);
        assert_eq!(t.specs_of(arrow).count(), 2);
        assert_eq!(t.specs_of(arrow).filter(|(_, r)| r.flags & SPEC_BUILTIN != 0).count(), 1);
        // Non-dependent built-in operator entry (WP-3701): distinct flag bit
        // from `REF_UNRESOLVED` and `REF_GROUP`.
        assert_eq!(
            REF_BUILTIN
                & (REF_DEPENDENT
                    | REF_UNRESOLVED
                    | REF_MEMBER
                    | REF_REWRITTEN
                    | REF_REVERSED
                    | REF_GROUP),
            0
        );
        let bi = t
            .push(WrittenRef {
                role: RefRole::Operator,
                flags: REF_BUILTIN,
                how: RefLookup::Builtin,
                ..entry(None)
            })
            .unwrap();
        assert_eq!(t.get(bi).flags & REF_BUILTIN, REF_BUILTIN);
        assert_eq!(t.get(bi).flags & REF_UNRESOLVED, 0);
        assert_eq!(t.get(bi).how, RefLookup::Builtin);
    }

    fn el(decl: u32, found: u32) -> FoundDecl {
        FoundDecl { decl: DeclId::new(decl), found: DeclId::new(found) }
    }

    fn dependent() -> WrittenRef {
        WrittenRef { flags: REF_DEPENDENT, ..entry(None) }
    }

    /// spec/ast.md §18.12: sets are interned by content (elements, how,
    /// flags); an entry's set is read back; the entry stays 28 bytes.
    #[test]
    fn found_sets_are_interned() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let a = t.push(dependent()).unwrap();
        let b = t.push(dependent()).unwrap();
        let third = t.push(dependent()).unwrap();
        let s1 = t.intern_found(&[el(2, 2), el(3, 9)], RefLookup::Unqualified, FOUND_ADL);
        let s2 = t.intern_found(&[el(2, 2), el(3, 9)], RefLookup::Unqualified, FOUND_ADL);
        assert_eq!(s1, s2);
        let closed = t.intern_found(&[el(2, 2), el(3, 9)], RefLookup::Unqualified, 0);
        assert_ne!(s1, closed, "flags are part of the set");
        let empty = t.intern_found(&[], RefLookup::Unqualified, FOUND_ADL);
        assert_eq!(t.found_set_count(), 3);
        t.set_found(a, s1);
        t.set_found(b, empty);
        t.set_found(third, closed);
        assert_eq!(t.found_set_id(a), t.found_set_id(a));
        let set = t.found_at_definition(a).unwrap();
        assert_eq!(set.decls, &[el(2, 2), el(3, 9)]);
        assert!(set.adl() && !set.is_closed() && set.contains(DeclId::new(9)));
        // Lookup ran and found nothing: `Some(empty)`, not `None`.
        let none = t.found_at_definition(b).unwrap();
        assert!(none.is_empty() && none.adl());
        assert!(t.found_at_definition(third).unwrap().is_closed());
        let finding: Vec<_> = t.entries_finding(DeclId::new(3)).collect();
        assert_eq!(finding, [a, third]);
        assert_eq!(t.entries_finding(DeclId::new(9)).count(), 2, "the using-shadow finds too");
        assert_eq!(t.entries_finding(DeclId::new(4)).count(), 0);
        assert!(t.found_mem().0 > 0);
        assert_eq!(size_of::<WrittenRef>(), 28);
        assert_eq!(size_of::<FoundDecl>(), 8);
        assert_eq!(size_of::<FoundSetRec>(), 12);
    }

    /// A callee recorded dependent and resolved at the definition loses
    /// its set; a row set out of order stays sorted; truncation drops the
    /// rows of the entries it drops.
    #[test]
    fn found_rows_follow_their_entries() {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        let s = t.intern_found(&[el(2, 2)], RefLookup::Unqualified, FOUND_ADL);
        let outer = t.push(dependent()).unwrap();
        t.set_found(outer, s);
        let inner = t.push(dependent()).unwrap();
        t.set_found(inner, s);
        let x = ExprId::new(4);
        t.defer(inner, x);
        assert_eq!(t.resolve(x, DeclId::new(2), None), Some(inner));
        assert_eq!(t.found_at_definition(inner), None, "popped by resolve");
        assert_eq!(t.found_rows().len(), 1);
        // Not the last row: removed all the same.
        let later = t.push(dependent()).unwrap();
        t.set_found(later, s);
        t.set_target(outer, None, None, REF_UNRESOLVED);
        assert_eq!(t.found_at_definition(outer), None);
        assert_eq!(t.found_rows(), &[(later, s)]);
        // Out of order (a carried table's): kept sorted, replaced once.
        t.set_found(outer, s);
        t.set_found(outer, s);
        t.set_target(outer, None, None, REF_DEPENDENT);
        let ids: Vec<_> = t.found_rows().iter().map(|r| r.0).collect();
        assert_eq!(ids, [outer, later]);
        t.truncate(RefMark { entries: 2, decls: 0 });
        assert_eq!(t.found_rows(), &[(outer, s)]);
        assert_eq!(t.found_set_count(), 1, "the sets stay interned");
    }

    #[test]
    fn off_records_no_found_set() {
        let mut t = WrittenRefs::default();
        let s = t.intern_found(&[el(2, 2)], RefLookup::Unqualified, 0);
        t.set_found(WrittenRefId::from_index(0), s);
        assert!(t.found_rows().is_empty());
        let mut t = WrittenRefs::default();
        assert_eq!(t.found_mem(), (0, 0));
        // Names parsed for the table alone (§18.8) get none either.
        t.set_on(true);
        let id = t.push(dependent()).unwrap();
        let was = t.set_pattern_nodes(false);
        let s = t.intern_found(&[el(2, 2)], RefLookup::Unqualified, 0);
        t.set_found(id, s);
        t.set_pattern_nodes(was);
        assert_eq!(t.found_at_definition(id), None);
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
