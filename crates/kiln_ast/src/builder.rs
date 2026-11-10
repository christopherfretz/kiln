//! [`AstBuilder`]: typed constructors that append nodes to an [`Ast`],
//! computing what the spec says is computed at construction (spec/ast.md
//! §12 WP A): expression dependence (§5.1) and value category (§5.2),
//! member lists, lookup-table entries (§4.3), redeclaration groups (§4.4)
//! and the declared types of tag and typedef declarations
//! (spec/types.md §13).
//!
//! The builder trusts its caller (sema, the dump loader, tests) for
//! *semantic* validity — it does not diagnose. It panics only on violations
//! of the AST's own invariants (a payload of the wrong kind, a member added
//! to a complete class, a pointer-to-member operator without a value
//! category), which are compiler bugs, never user input (spec/ast.md §2.4).
#![warn(missing_docs)]

use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::ast::Ast;
use crate::decl::{
    Access, CtorInit, DeclFlags, DeclKind, DeclMisc, DefinitionBits, EnumData, EnumeratorData,
    FieldData, FnSpec, FriendKind, FriendTarget, FunctionData, LetData, LinkageLang, Members,
    NamespaceData, NewDecl, ParamData, Payload, PayloadClass, RecordData, Redecl, ScopeData,
    StaticAssertData, TagKind, TemplLink, TypedefData, VarData, VarEval, VarSpec, VarStorage,
};
use crate::expr::{
    ArrayTraitExtra, BinaryOp, CastKind, CharKind, CoawaitParts, CompoundAssignExtra,
    ConstructInfo, ConstructionKind, DeclRefExtra, ExprDeps, ExprFlags, ExprKind, ExprNode,
    GenericAssoc, GenericControl, InitListExtra, NewExtra, NewInitStyle, NonOdrUse, ObjectKind,
    Radix, StorageDuration, StringKind, SuspendKind, TraitKind, TypeTrait, TypeidOperand, UnaryOp,
    ValueCategory,
};
use crate::ids::{
    ArgListId, AttrId, ConstValueId, DeclId, ExprId, LambdaId, ListId, QualType, RawId, StmtId,
    StringLitId, TypeId,
};
use crate::lookup::LookupFlags;
use crate::names::DeclName;
use crate::slot::AlreadySet;
use crate::stmt::{
    AsmData, AsmOperand, CaseData, CoroutineBodyData, ExpansionData, IfData, IfKind, LoopData,
    RangeForData, StmtKind, StmtNode, SwitchData,
};
use crate::types::TypeFlags;

/// Builtin `TypeId` of the `Dependent` placeholder (spec/types.md §5).
pub const BUILTIN_DEPENDENT: u32 = 29;

/// The invariant behind every side-table insert keyed by an expression
/// the builder has just pushed: arena ids are never reused, so a fresh
/// id has no entry yet (spec/ast.md §2.1; WP-972 audit, class (a)).
pub(crate) const FRESH_EXPR: &str =
    "invariant: a just-pushed ExprId has no side-table entry (arena ids are never reused)";

/// [`FRESH_EXPR`] for declarations: a just-pushed declaration has no
/// lexical context and no side-table entry yet.
pub(crate) const FRESH_DECL: &str =
    "invariant: a just-pushed DeclId has no lexical context or side-table entry yet";

/// The header fields shared by every declaration constructor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeclHead {
    /// The semantic context.
    pub ctx: DeclId,
    /// The lexical context when it differs from `ctx` (out-of-line
    /// definitions, friends): sets `HAS_LEXICAL_CTX` and decides which
    /// member list the declaration joins.
    pub lexical: Option<DeclId>,
    /// The name (`DeclName::EMPTY` for unnamed declarations).
    pub name: DeclName,
    /// The name's location.
    pub loc: Loc,
    /// Initial flags (`IMPLICIT`, `INVALID`, `DEPENDENT_CTX`, ...;
    /// `HAS_LEXICAL_CTX` and `HAS_ATTRS` are set by the builder).
    pub flags: DeclFlags,
    /// Access, friend kind, module ownership, P2169 name independence.
    pub misc: DeclMisc,
}

impl DeclHead {
    /// A head with no flags, default `misc` and `lexical == ctx`.
    pub const fn new(ctx: DeclId, name: DeclName, loc: Loc) -> Self {
        DeclHead { ctx, lexical: None, name, loc, flags: DeclFlags::empty(), misc: DeclMisc::new() }
    }

    /// With additional flags.
    #[must_use]
    pub const fn flags(mut self, f: DeclFlags) -> Self {
        self.flags = self.flags.with(f);
        self
    }

    /// With `misc`.
    #[must_use]
    pub const fn misc(mut self, m: DeclMisc) -> Self {
        self.misc = m;
        self
    }

    /// With an access specifier.
    #[must_use]
    pub const fn access(mut self, a: Access) -> Self {
        self.misc = self.misc.with_access(a);
        self
    }

    /// With a lexical context different from `ctx`.
    #[must_use]
    pub const fn lexical(mut self, lexical: DeclId) -> Self {
        self.lexical = Some(lexical);
        self
    }
}

/// Typed constructors over a borrowed [`Ast`] (spec/ast.md §12, WP A).
#[derive(Debug)]
pub struct AstBuilder<'a> {
    pub(crate) ast: &'a mut Ast,
}

/// Maps type flags to expression dependence (spec/ast.md §5.1): a dependent
/// type makes an expression type-, value- and instantiation-dependent.
fn deps_of_type_flags(f: TypeFlags) -> ExprDeps {
    let mut d = ExprDeps::NONE;
    if f.contains(TypeFlags::DEPENDENT) {
        d |= ExprDeps::TYPE_VALUE_INST;
    }
    if f.contains(TypeFlags::INSTANTIATION_DEPENDENT) {
        d |= ExprDeps::INSTANTIATION;
    }
    if f.contains(TypeFlags::UNEXPANDED_PACK) {
        d |= ExprDeps::UNEXPANDED_PACK;
    }
    if f.contains(TypeFlags::CONTAINS_ERROR) {
        d |= ExprDeps::ERROR;
    }
    d
}

impl<'a> AstBuilder<'a> {
    /// A builder appending to `ast`.
    pub fn new(ast: &'a mut Ast) -> Self {
        AstBuilder { ast }
    }

    /// The AST being built.
    pub fn ast(&self) -> &Ast {
        self.ast
    }

    /// The AST being built, mutably (for late setters on the arenas).
    pub fn ast_mut(&mut self) -> &mut Ast {
        self.ast
    }

    /// A `ListId` holding `ids`.
    pub fn list<I: RawId>(&mut self, ids: &[I]) -> ListId {
        self.ast.lists.push(ids)
    }

    // =====================================================================
    // Declarations
    // =====================================================================

    /// Pushes a declaration, links it into its lexical context's member
    /// list and registers its name (§4.3). `prev` is the redeclared
    /// declaration, if any.
    pub(crate) fn add(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: Option<QualType>,
        payload: Payload,
        prev: Option<DeclId>,
    ) -> DeclId {
        // A template pattern (spec/sema-templates.md §4.1) is neither a
        // member nor a name: its template decl is both.
        if std::mem::take(&mut self.ast.templates.pattern_next) {
            return self.add_pattern(h, kind, ty, payload);
        }
        let h = self.ast.templates.dependent_head(h);
        let d = self.ast.decls.push(
            NewDecl {
                kind,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty,
            },
            payload,
        );
        if let Some(lex) = h.lexical.filter(|&l| l != h.ctx) {
            self.ast.decls.set_lexical_ctx(d, lex).expect(FRESH_DECL);
        }
        let lexical = h.lexical.unwrap_or(h.ctx);
        if self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, d);
        }
        let adl_only = h.misc.friend() == FriendKind::Undeclared;
        self.register_name(h.ctx, h.name, d, prev, h.misc.name_independent(), adl_only);
        d
    }

    /// Pushes a declaration and links it into its lexical context's
    /// member list without entering its name in any lookup table.
    pub(crate) fn add_unregistered(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        payload: Payload,
        member: bool,
    ) -> DeclId {
        let h = self.ast.templates.dependent_head(h);
        let d = self.ast.decls.push(
            NewDecl {
                kind,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty: None,
            },
            payload,
        );
        if let Some(lex) = h.lexical.filter(|&l| l != h.ctx) {
            self.ast.decls.set_lexical_ctx(d, lex).expect(FRESH_DECL);
        }
        let lexical = h.lexical.unwrap_or(h.ctx);
        if member && self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, d);
        }
        d
    }

    /// The builtin template `name` (`__make_integer_seq`,
    /// `__type_pack_element`; spec/sema-templates.md §9.3) in `ctx`:
    /// implicit, entered in `ctx`'s lookup table but not in its member
    /// list (clang's builtin template decls are not printed). `which`
    /// is the inline payload naming the builtin.
    pub fn builtin_template(&mut self, ctx: DeclId, name: DeclName, which: u32) -> DeclId {
        let h = DeclHead::new(ctx, name, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let d = self.add_unregistered(h, DeclKind::BuiltinTemplate, Payload::None(which), false);
        self.register_name(ctx, name, d, None, false, false);
        d
    }

    /// A using-declarator `using qualifier name;` ([namespace.udecl]):
    /// `h.name` is the member name, `qualifier` the nested-name-specifier
    /// as written. The declaration itself is not entered in lookup; its
    /// [`AstBuilder::using_shadow`]s are.
    pub fn using_decl(&mut self, h: DeclHead, qualifier: Option<crate::ids::NnsId>) -> DeclId {
        let q = qualifier.map_or(0, RawId::raw);
        self.add_unregistered(h, DeclKind::Using, Payload::None(q), true)
    }

    /// The implicit `UsingShadow` a using-declaration (or `using enum`)
    /// introduces for `target` into `h.ctx` ([namespace.udecl]/1). It is
    /// entered in lookup, replacing `prev` (an earlier shadow of the same
    /// target from a repeated using-declaration).
    pub fn using_shadow(&mut self, h: DeclHead, target: DeclId, prev: Option<DeclId>) -> DeclId {
        let d = self.add_unregistered(
            h.flags(DeclFlags::IMPLICIT),
            DeclKind::UsingShadow,
            Payload::None(target.raw()),
            true,
        );
        self.register_name(h.ctx, h.name, d, prev, false, false);
        d
    }

    /// The implicit `ConstructorUsingShadow` an inheriting-constructor
    /// using-declaration (`using B::B;`, [namespace.udecl]/3) introduces
    /// into class `h.ctx` for constructor (or constructor template)
    /// `target` of a base class. `nominated` is the base class's own shadow
    /// when the base inherited `target` itself. Appended to the member list
    /// but not entered in lookup: overload resolution for the class's
    /// constructors collects the shadows from the member list (WP-114).
    pub fn constructor_using_shadow(
        &mut self,
        h: DeclHead,
        target: DeclId,
        nominated: Option<DeclId>,
    ) -> DeclId {
        let d = self.add_unregistered(
            h.flags(DeclFlags::IMPLICIT),
            DeclKind::ConstructorUsingShadow,
            Payload::None(target.raw()),
            true,
        );
        if let Some(n) = nominated {
            let _ = self.ast.decls.cold.ctor_shadow_nominated.insert(d, n);
        }
        d
    }

    /// A using-declarator with a dependent qualifier in a class template
    /// (clang `UnresolvedUsingValueDecl`): `h.name` is the name as
    /// written, `nns` the qualifier. Entered in lookup when `register`
    /// (a member name; uses of it are dependent), not for an
    /// inheriting-constructor declarator.
    pub fn unresolved_using_value(
        &mut self,
        h: DeclHead,
        nns: crate::ids::NnsId,
        register: bool,
    ) -> DeclId {
        let d = self.add_unregistered(
            h,
            DeclKind::UnresolvedUsingValue,
            Payload::None(nns.raw()),
            true,
        );
        if register {
            self.register_name(h.ctx, h.name, d, None, false, false);
        }
        d
    }

    /// `using typename nns name;` with a dependent qualifier in a class
    /// template (clang `UnresolvedUsingTypenameDecl`, WP-179): a member
    /// type name entered in lookup whose `ty` is its own canonical,
    /// dependent [`TypeKind::UnresolvedUsing`](crate::types::TypeKind)
    /// type under keyword-less `Elaborated` sugar (clang's type for the
    /// name as written). Redone as a using-declaration at instantiation.
    pub fn unresolved_using_typename(&mut self, h: DeclHead, nns: crate::ids::NnsId) -> DeclId {
        let d = self.ast.decls.next_id();
        let t = QualType::unqual(self.ast.types.unresolved_using_type(d));
        let ty = self.ast.types.elaborated(crate::types::TagKeyword::None, None, t);
        self.add(h, DeclKind::UnresolvedUsingTypename, Some(ty), Payload::None(nns.raw()), None)
    }

    /// `using enum E;` ([enum.udecl]); `e` is the enumeration. Not
    /// entered in lookup (its enumerators' shadows are).
    pub fn using_enum(&mut self, h: DeclHead, e: DeclId) -> DeclId {
        self.add_unregistered(h, DeclKind::UsingEnum, Payload::None(e.raw()), true)
    }

    /// `namespace h.name = target;` ([namespace.alias]); `target` is the
    /// namespace or namespace alias as written. A redeclaration of the
    /// same alias replaces `prev` in lookup.
    pub fn namespace_alias(&mut self, h: DeclHead, target: DeclId, prev: Option<DeclId>) -> DeclId {
        let d =
            self.add_unregistered(h, DeclKind::NamespaceAlias, Payload::None(target.raw()), true);
        self.register_name(h.ctx, h.name, d, prev, false, false);
        d
    }

    /// A file-scope `asm("...")` declaration ([dcl.asm]); the inline
    /// payload is the template string.
    pub fn file_scope_asm(&mut self, ctx: DeclId, loc: Loc, string: ExprId) -> DeclId {
        let h = DeclHead::new(ctx, DeclName::EMPTY, loc);
        self.add_unregistered(h, DeclKind::FileScopeAsm, Payload::None(string.raw()), true)
    }

    /// The placeholder target of a `using_if_exists` using-declaration
    /// whose name was not found (clang's `UnresolvedUsingIfExistsDecl`):
    /// in no member list and no lookup table; its shadow is.
    pub fn unresolved_using_if_exists(&mut self, h: DeclHead) -> DeclId {
        self.add_unregistered(h, DeclKind::UnresolvedUsingIfExists, Payload::None(0), false)
    }

    /// Inserts `d` into the lookup table of `ctx` (after skipping
    /// `LinkageSpec`/`Export`), then into enclosing tables for unscoped
    /// enumerations (their enumerators) and inline namespaces (§4.3).
    /// A friend first declared by its friend declaration
    /// ([`FriendKind::Undeclared`]) is entered with
    /// [`LookupFlags::ADL_ONLY`] ([namespace.memdef]/3).
    fn register_name(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        d: DeclId,
        prev: Option<DeclId>,
        name_independent: bool,
        adl_only: bool,
    ) {
        if name.is_empty() {
            return;
        }
        // P2169: each name-independent declaration is a separate entry.
        let replaces = if name_independent { None } else { prev };
        let mut via_inline = false;
        let mut cur = self.ast.redecl_context(ctx);
        loop {
            if !has_lookup_table(self.ast.decls.get(cur).kind()) {
                return;
            }
            let table = self.ast.primary_context(cur);
            let mut flags = if adl_only { LookupFlags::ADL_ONLY } else { LookupFlags::NONE };
            if via_inline {
                flags = flags.with(LookupFlags::VIA_INLINE);
            }
            self.ast.lookup.insert_with_flags(table, name, d, replaces, flags);
            let next = if self.ast.is_transparent(table) {
                // Unscoped enumeration: enumerators are also members of
                // the enclosing scope ([dcl.enum]/11).
                via_inline = false;
                self.ast.decls.get(table).ctx()
            } else if self.is_inline_namespace(table) {
                via_inline = true;
                self.ast.decls.get(table).ctx()
            } else {
                None
            };
            match next {
                Some(n) => cur = self.ast.redecl_context(n),
                None => return,
            }
        }
    }

    /// Takes `d` out of every lookup table [`AstBuilder::add`] entered
    /// its name in (the context and the enclosing ones an unscoped
    /// enumeration or inline namespace reaches): a redefined class or
    /// enumeration is not visible to later lookups (spec/sema.md §4.4).
    pub fn unregister_name(&mut self, d: DeclId) {
        let n = self.ast.decls.get(d);
        let (name, Some(ctx)) = (n.name(), n.ctx()) else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let mut cur = self.ast.redecl_context(ctx);
        for _ in 0..10_000 {
            if !has_lookup_table(self.ast.decls.get(cur).kind()) {
                return;
            }
            let table = self.ast.primary_context(cur);
            self.ast.lookup.remove(table, name, d);
            let next = if self.ast.is_transparent(table) || self.is_inline_namespace(table) {
                self.ast.decls.get(table).ctx()
            } else {
                None
            };
            match next {
                Some(n) => cur = self.ast.redecl_context(n),
                None => return,
            }
        }
    }

    fn is_inline_namespace(&self, ns: DeclId) -> bool {
        self.ast.decls.namespace(self.ast.first(ns)).is_some_and(NamespaceData::is_inline)
    }

    /// The redeclaration link of a new declaration: joins `prev`'s group
    /// (after its latest declaration) or starts a group at the id the next
    /// push returns.
    pub(crate) fn redecl_link(&mut self, prev: Option<DeclId>) -> Redecl {
        let d = self.ast.decls.next_id();
        match prev {
            Some(p) => {
                let group = self
                    .ast
                    .decls
                    .redecl(p)
                    .expect("invariant: Sema passes a redeclarable `prev` (lookup filters by kind)")
                    .group;
                let latest = self.ast.redecls.get(group).latest;
                self.ast.redecls.push_redecl(group, d);
                if self.ast.modules.active {
                    self.ast.modules.links.push((d, latest));
                }
                Redecl { prev: Some(latest), group }
            }
            None => Redecl { prev: None, group: self.ast.redecls.new_group(d) },
        }
    }

    /// The latest declaration of `prev`'s entity (what lookup holds).
    fn latest_of(&self, prev: Option<DeclId>) -> Option<DeclId> {
        prev.map(|p| self.ast.latest(p))
    }

    /// A namespace definition `namespace N { }` ([namespace.def]); `prev`
    /// is the namespace it reopens. `inline` applies to the original
    /// definition (a reopening may repeat but not add it,
    /// [namespace.def]/3; sema diagnoses). `name` is empty for an unnamed
    /// namespace.
    pub fn namespace(&mut self, h: DeclHead, inline: bool, prev: Option<DeclId>) -> DeclId {
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let mut flags = 0;
        if inline {
            flags |= NamespaceData::INLINE;
        }
        if h.name.is_empty() {
            flags |= NamespaceData::ANONYMOUS;
        }
        let payload =
            Payload::Namespace(NamespaceData { redecl, members: Members::default(), flags });
        let d = self.add(h, DeclKind::Namespace, None, payload, replaces);
        if prev.is_none() && inline {
            let parent = self.ast.primary_context(self.ast.redecl_context(h.ctx));
            self.ast.lookup.add_inline_child(parent, d);
        }
        d
    }

    /// A namespace that is neither entered in `ctx`'s lookup table nor
    /// listed among its members: a context for naming only (clang's
    /// implicit `namespace std` around `AArch64`'s builtin `__va_list`).
    pub fn hidden_namespace(&mut self, ctx: DeclId, name: DeclName) -> DeclId {
        let redecl = self.redecl_link(None);
        let payload =
            Payload::Namespace(NamespaceData { redecl, members: Members::default(), flags: 0 });
        let h = DeclHead::new(ctx, name, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        self.add_unregistered(h, DeclKind::Namespace, payload, false)
    }

    /// A linkage specification `extern "C" { }` ([dcl.link]). Transparent:
    /// its declarations are named in the enclosing context.
    pub fn linkage_spec(&mut self, ctx: DeclId, lang: LinkageLang, loc: Loc) -> DeclId {
        self.linkage_spec_head(DeclHead::new(ctx, DeclName::EMPTY, loc), lang)
    }

    /// A linkage specification with head `h` (its name is ignored; an
    /// implicit one surrounds a lazily declared builtin).
    pub fn linkage_spec_head(&mut self, h: DeclHead, lang: LinkageLang) -> DeclId {
        let payload = Payload::Scope(ScopeData { members: Members::default(), aux: lang as u32 });
        let h = DeclHead { name: DeclName::EMPTY, ..h };
        self.add(h, DeclKind::LinkageSpec, None, payload, None)
    }

    /// An export declaration `export { }` ([module.interface]).
    /// Transparent like a linkage specification.
    pub fn export_decl(&mut self, ctx: DeclId, loc: Loc) -> DeclId {
        let payload = Payload::Scope(ScopeData { members: Members::default(), aux: 0 });
        self.add(DeclHead::new(ctx, DeclName::EMPTY, loc), DeclKind::Export, None, payload, None)
    }

    /// A class declaration ([class.pre]); `prev` is the redeclared class.
    /// The first declaration gets a new `Record` type (spec/types.md
    /// `record_type`); redeclarations share it. Call
    /// [`AstBuilder::begin_record_definition`] for a definition.
    pub fn record(&mut self, h: DeclHead, tag: TagKind, prev: Option<DeclId>) -> DeclId {
        self.record_in(h, tag, prev, false)
    }

    /// A lambda's closure type ([expr.prim.lambda.closure]/1), as
    /// [`AstBuilder::record`]. `never_dependent`: its type is not
    /// dependent even inside a template's head (clang
    /// `LDK_NeverDependent`; spec/sema-lambda.md §9).
    pub fn closure_record(&mut self, h: DeclHead, never_dependent: bool) -> DeclId {
        self.record_in(h, TagKind::Class, None, never_dependent)
    }

    fn record_in(
        &mut self,
        h: DeclHead,
        tag: TagKind,
        prev: Option<DeclId>,
        never_dependent: bool,
    ) -> DeclId {
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let ty = if let Some(p) = prev {
            self.ast.decls.get(self.ast.first(p)).ty()
        } else {
            let d = self.ast.decls.next_id();
            let dep = !never_dependent && self.nested_in_template();
            Some(QualType::unqual(self.ast.types.record_type_dependent(d, dep)))
        };
        let payload = Payload::Record(RecordData {
            redecl,
            members: Members::default(),
            bases: ListId::EMPTY,
            defn: DefinitionBits::empty(),
            templ: TemplLink::NONE,
            lambda: None,
            tag,
        });
        self.add(h, DeclKind::CXXRecord, ty, payload, replaces)
    }

    /// Starts the definition of class `r` (at `{`): records `r` as its
    /// entity's definition, so member lookup uses `r`'s table.
    ///
    /// # Errors
    /// If the entity already has a definition (sema diagnoses
    /// redefinition, [basic.def.odr]/1, before calling).
    pub fn begin_record_definition(&mut self, r: DeclId) -> Result<(), AlreadySet> {
        self.ast.set_definition(r)?;
        self.ast.decls.begin_definition(r);
        Ok(())
    }

    /// Completes the definition of class `r` (at `}`) with its
    /// `DefinitionData` bits; the member list is frozen from here (§2.4).
    pub fn complete_record_definition(&mut self, r: DeclId, bits: DefinitionBits) {
        self.ast.decls.set_definition_bits(r, bits);
        self.ast.decls.complete_definition(r);
    }

    /// A non-static data member ([class.mem]) of class `h.ctx`.
    pub fn field(
        &mut self,
        h: DeclHead,
        ty: QualType,
        bit_width: Option<ExprId>,
        flags: u32,
    ) -> DeclId {
        let payload = Payload::Field(FieldData { bit_width, init: None, flags });
        self.add(h, DeclKind::Field, Some(ty), payload, None)
    }

    /// An `IndirectField`: the name of a member of an anonymous union or
    /// struct ([class.union.anon]) injected into the enclosing class (or
    /// namespace scope / block, `h.ctx`). `chain` is the path of `Field`s
    /// from `h.ctx` to the member (anonymous-member fields first).
    pub fn indirect_field(&mut self, h: DeclHead, ty: QualType, chain: &[DeclId]) -> DeclId {
        let list = self.ast.lists.push(chain);
        self.add(
            h.flags(DeclFlags::IMPLICIT),
            DeclKind::IndirectField,
            Some(ty),
            Payload::None(list.raw()),
            None,
        )
    }

    /// An enumeration declaration ([dcl.enum]); `flags` are
    /// [`EnumData`]'s `SCOPED` / `SCOPED_STRUCT` (`FIXED` is implied by
    /// `fixed`). `fixed` is `(underlying, promotion)` for an enum-base
    /// ([dcl.enum]/5, [conv.prom]/4). The first declaration gets a new
    /// `Enum` type; redeclarations share it.
    pub fn enumeration(
        &mut self,
        h: DeclHead,
        flags: u32,
        fixed: Option<(QualType, QualType)>,
        prev: Option<DeclId>,
    ) -> DeclId {
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let ty = if let Some(p) = prev {
            self.ast.decls.get(self.ast.first(p)).ty()
        } else {
            let d = self.ast.decls.next_id();
            let dep = self.nested_in_template();
            Some(QualType::unqual(self.ast.types.enum_type_dependent(d, dep)))
        };
        let mut flags = flags & (EnumData::SCOPED | EnumData::SCOPED_STRUCT);
        if fixed.is_some() {
            flags |= EnumData::FIXED;
        }
        let payload = Payload::Enum(EnumData {
            redecl,
            members: Members::default(),
            int_type: fixed.map(|f| f.0),
            promotion_type: fixed.map(|f| f.1),
            flags,
            templ: TemplLink::NONE,
        });
        self.add(h, DeclKind::Enum, ty, payload, replaces)
    }

    /// Starts the definition of enumeration `e` (at `{`).
    ///
    /// # Errors
    /// If the entity already has a definition.
    pub fn begin_enum_definition(&mut self, e: DeclId) -> Result<(), AlreadySet> {
        self.ast.set_definition(e)?;
        self.ast.decls.begin_enum_definition(e);
        Ok(())
    }

    /// Completes the definition of `e` (at `}`). `computed` is `(underlying,
    /// promotion)` for an enumeration without a fixed underlying type
    /// ([dcl.enum]/7, [conv.prom]/3); `None` when it is fixed.
    ///
    /// # Errors
    /// If `computed` is given for an enumeration whose types are already
    /// set (a fixed one).
    pub fn complete_enum_definition(
        &mut self,
        e: DeclId,
        computed: Option<(QualType, QualType)>,
    ) -> Result<(), AlreadySet> {
        if let Some((int_type, promotion)) = computed {
            self.ast.decls.set_enum_int_type(e, int_type, promotion)?;
        }
        self.ast.decls.complete_enum_definition(e);
        Ok(())
    }

    /// Whether the tag being declared is nested in a template (not the
    /// pattern itself, whose type is its injected-class-name): its type is
    /// dependent, a member of the current instantiation ([temp.dep.type]/9).
    fn nested_in_template(&self) -> bool {
        self.ast.templates.dependent_depth > 0 && !self.ast.templates.pattern_next
    }

    /// An enumerator of enumeration `h.ctx` ([dcl.enum]/2). Its type is the
    /// enumeration's; the value is set with
    /// [`crate::decl::DeclArena::set_enumerator_value`]. Enumerators of
    /// unscoped enumerations are also named in the enclosing context.
    pub fn enumerator(&mut self, h: DeclHead, init: Option<ExprId>) -> DeclId {
        let ty = self.ast.decls.get(h.ctx).ty();
        let payload = Payload::Enumerator(EnumeratorData { value: None, init });
        self.add(h, DeclKind::EnumConstant, ty, payload, None)
    }

    /// A `typedef` ([dcl.typedef]) or alias declaration (`kind` =
    /// `Typedef` / `TypeAlias`). Each declaration gets its own `Typedef`
    /// sugar type; `ty` of the decl is that type.
    ///
    /// # Panics
    /// If `kind` is not `Typedef` or `TypeAlias`.
    pub fn typedef(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        underlying: QualType,
        prev: Option<DeclId>,
    ) -> DeclId {
        assert!(
            matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias),
            "not a typedef kind: {kind:?}"
        );
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let d = self.ast.decls.next_id();
        let ty = QualType::unqual(self.ast.types.typedef_type(d, underlying));
        let payload = Payload::Typedef(TypedefData { redecl, underlying, templ: TemplLink::NONE });
        self.add(h, kind, Some(ty), payload, replaces)
    }

    /// An alias declaration (`TypeAlias`) of `underlying` linked into no
    /// context's member list and no lookup table: Sema's dependent-scope
    /// marker for a dependent `decltype(e)::` qualifier
    /// (spec/sema-templates.md §3, WP-119). Its `ty` is its own `Typedef`
    /// sugar type, as for [`AstBuilder::typedef`].
    pub fn unlinked_alias(&mut self, h: DeclHead, underlying: QualType) -> DeclId {
        let redecl = self.redecl_link(None);
        let d = self.ast.decls.next_id();
        let ty = QualType::unqual(self.ast.types.typedef_type(d, underlying));
        let payload = Payload::Typedef(TypedefData { redecl, underlying, templ: TemplLink::NONE });
        self.ast.decls.push(
            NewDecl {
                kind: DeclKind::TypeAlias,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty: Some(ty),
            },
            payload,
        )
    }

    /// A function declaration of `kind` (`Function`, `CXXMethod`,
    /// `CXXConstructor`, `CXXDestructor`, `CXXConversion`) with function
    /// type `ty`; `prev` is the redeclared function (overloads are distinct
    /// entities, `prev = None`). Parameters are created afterwards with
    /// [`AstBuilder::param`] and attached with [`AstBuilder::set_params`].
    ///
    /// # Panics
    /// If `kind` is not a redeclarable function kind.
    pub fn function(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: QualType,
        spec: FnSpec,
        prev: Option<DeclId>,
    ) -> DeclId {
        assert!(
            kind.payload() == PayloadClass::Function && kind != DeclKind::CXXDeductionGuide,
            "not a function kind: {kind:?}"
        );
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let payload = Payload::Function(FunctionData {
            redecl,
            params: ListId::EMPTY,
            body: None,
            spec,
            templ: TemplLink::NONE,
        });
        self.add(h, kind, Some(ty), payload, replaces)
    }

    /// A deduction guide ([temp.deduct.guide]) with function type `ty`
    /// (its return type is the guided specialization). Guides are not
    /// redeclarable; parameters are attached as for [`AstBuilder::function`].
    pub fn deduction_guide(&mut self, h: DeclHead, ty: QualType, spec: FnSpec) -> DeclId {
        let payload = Payload::Function(FunctionData {
            redecl: self.redecl_link(None),
            params: ListId::EMPTY,
            body: None,
            spec,
            templ: TemplLink::NONE,
        });
        self.add(h, DeclKind::CXXDeductionGuide, Some(ty), payload, None)
    }

    /// A function parameter of `func` ([dcl.fct]); `index` is its position.
    /// Parameters are not members of any list or lookup table (§4.3).
    pub fn param(
        &mut self,
        func: DeclId,
        name: DeclName,
        loc: Loc,
        ty: QualType,
        index: u16,
        flags: u16,
    ) -> DeclId {
        self.param_head(DeclHead::new(func, name, loc), ty, index, flags)
    }

    /// A parameter created while its function declarator is parsed, so
    /// that it is in scope in the rest of the declarator
    /// ([basic.scope.param]). Its context is provisionally `ctx` (the
    /// context the declarator appears in) until the function adopts it
    /// ([`crate::decl::DeclArena::set_param_owner`]); like every
    /// parameter it joins no member list or lookup table (§4.3), and it
    /// does not consume a pending template-pattern mark.
    pub fn prototype_param(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        loc: Loc,
        ty: QualType,
        index: u16,
        flags: u16,
    ) -> DeclId {
        let h = self.ast.templates.dependent_head(DeclHead::new(ctx, name, loc));
        let payload =
            Payload::Param(ParamData { default_arg: None, original_ty: None, index, flags });
        self.ast.decls.push(
            NewDecl {
                kind: DeclKind::ParmVar,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty: Some(ty),
            },
            payload,
        )
    }

    /// An init-capture's variable ([expr.prim.lambda.capture]/6), created
    /// at the *lambda-introducer* so that it is in scope in the
    /// *lambda-declarator* (P2036). Its context is provisionally `ctx` (the
    /// context the lambda appears in) until the call operator adopts it
    /// ([`crate::decl::DeclArena::set_init_capture_owner`]); like a
    /// prototype parameter it joins no member list or lookup table.
    pub fn init_capture_var(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        loc: Loc,
        ty: QualType,
        name_independent: bool,
    ) -> DeclId {
        let misc = DeclMisc::new().with_name_independent(name_independent);
        let h = self.ast.templates.dependent_head(DeclHead::new(ctx, name, loc).misc(misc));
        let payload = Payload::Var(VarData {
            redecl: self.redecl_link(None),
            init: None,
            spec: VarSpec::new(),
            eval: VarEval::default(),
            templ: TemplLink::NONE,
        });
        self.ast.decls.push(
            NewDecl {
                kind: DeclKind::Var,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty: Some(ty),
            },
            payload,
        )
    }

    /// A parameter declared by `head` (whose context is the function), for
    /// implicit parameters (`DeclFlags::IMPLICIT`).
    pub fn param_head(&mut self, head: DeclHead, ty: QualType, index: u16, flags: u16) -> DeclId {
        let payload =
            Payload::Param(ParamData { default_arg: None, original_ty: None, index, flags });
        self.add(head, DeclKind::ParmVar, Some(ty), payload, None)
    }

    /// Sets the parameter list of `func`.
    ///
    /// # Errors
    /// If a non-empty list was already set.
    pub fn set_params(&mut self, func: DeclId, params: &[DeclId]) -> Result<(), AlreadySet> {
        let list = self.ast.lists.push(params);
        self.ast.decls.set_params(func, list, params)
    }

    /// Sets the body of `func` and records it as its entity's definition.
    ///
    /// # Errors
    /// If `func` already has a body or the entity a definition.
    pub fn set_body(&mut self, func: DeclId, body: StmtId) -> Result<(), AlreadySet> {
        self.ast.decls.set_body(func, body)?;
        self.ast.set_definition(func)
    }

    /// A variable declaration ([basic.pre]/6, [dcl.dcl]); `prev` is the
    /// redeclared variable. The initializer is set with
    /// [`AstBuilder::set_var_init`].
    pub fn var(
        &mut self,
        h: DeclHead,
        ty: QualType,
        spec: VarSpec,
        prev: Option<DeclId>,
    ) -> DeclId {
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let payload = Payload::Var(VarData {
            redecl,
            init: None,
            spec,
            eval: VarEval::default(),
            templ: TemplLink::NONE,
        });
        self.add(h, DeclKind::Var, Some(ty), payload, replaces)
    }

    /// One structured binding ([dcl.struct.bind]/1, clang's
    /// `BindingDecl`): typed and given its binding expression when the
    /// decomposition's initializer is known (`DeclArena::set_binding`). At
    /// namespace scope it joins the member list ahead of its
    /// decomposition variable, as in clang.
    pub fn binding(&mut self, h: DeclHead) -> DeclId {
        self.add(h, DeclKind::Binding, None, Payload::None(0), None)
    }

    /// The implicit variable holding a tuple-like structured binding's
    /// value ([dcl.struct.bind]/4, clang's holding `VarDecl`): a member
    /// of its lexical context that name lookup never finds.
    pub fn holding_var(&mut self, h: DeclHead, ty: QualType, spec: VarSpec) -> DeclId {
        let payload = Payload::Var(VarData {
            redecl: self.redecl_link(None),
            init: None,
            spec,
            eval: VarEval::default(),
            templ: TemplLink::NONE,
        });
        let name = h.name;
        let mut h = h;
        h.name = DeclName::EMPTY;
        let d = self.add(h, DeclKind::Var, Some(ty), payload, None);
        self.ast.decls.set_name(d, name);
        d
    }

    /// Marks `v` (an unnamed `Var`) as the decomposition holding
    /// `bindings` (write-once).
    ///
    /// # Errors
    /// If `v` already has bindings.
    pub fn set_decomposition(&mut self, v: DeclId, bindings: &[DeclId]) -> Result<(), AlreadySet> {
        let list = self.ast.lists.push(bindings);
        self.ast.decls.set_decomp_bindings(v, list, bindings)
    }

    /// Sets the initializer of variable `v` (once).
    ///
    /// # Errors
    /// If already set.
    pub fn set_var_init(&mut self, v: DeclId, init: ExprId) -> Result<(), AlreadySet> {
        self.ast.decls.set_init(v, init)
    }

    /// A `static_assert` declaration ([dcl.pre]/10, P2741 messages).
    pub fn static_assert(
        &mut self,
        ctx: DeclId,
        loc: Loc,
        cond: ExprId,
        message: Option<ExprId>,
    ) -> DeclId {
        let payload = Payload::StaticAssert(StaticAssertData { cond, message });
        self.add(
            DeclHead::new(ctx, DeclName::EMPTY, loc),
            DeclKind::StaticAssert,
            None,
            payload,
            None,
        )
    }

    /// A consteval-block-declaration ([dcl.pre], P3289R1): `call` is the
    /// call of the block's synthesized `consteval` lambda.
    pub fn consteval_block(&mut self, ctx: DeclId, loc: Loc, call: ExprId) -> DeclId {
        let payload = Payload::StaticAssert(StaticAssertData { cond: call, message: None });
        self.add(
            DeclHead::new(ctx, DeclName::EMPTY, loc),
            DeclKind::ConstevalBlock,
            None,
            payload,
            None,
        )
    }

    /// Attaches attributes to `d` (sets `HAS_ATTRS`, §7).
    ///
    /// # Errors
    /// If `d` already has attributes.
    pub fn attach_attrs(&mut self, d: DeclId, attrs: &[AttrId]) -> Result<(), AlreadySet> {
        let list = self.ast.lists.push(attrs);
        self.ast.attrs.decl_attrs.insert(d, list)?;
        self.ast.decls.set_has_attrs(d);
        Ok(())
    }

    /// Removes attribute `a` from `d`'s list: one kept provisionally and
    /// found invalid once more is known (a class-head `transparent_union`
    /// whose fields fail the check at the closing brace).
    pub fn remove_attr(&mut self, d: DeclId, a: AttrId) {
        let old = self.ast.attrs.of_decl(d);
        let rest: Vec<AttrId> = self.ast.lists.iter(old).filter(|&x| x != a).collect();
        let list = self.ast.lists.push(&rest);
        let _ = self.ast.attrs.decl_attrs.refine(d, list);
    }

    /// Appends `attrs` to `d`'s attribute list (attaching one if `d` has
    /// none): for attributes that appertain to a declaration after it was
    /// first given its own (a class's trailing GNU attributes,
    /// `struct S { ... } __attribute__((packed));`).
    pub fn append_attrs(&mut self, d: DeclId, attrs: &[AttrId]) {
        if attrs.is_empty() {
            return;
        }
        let old = self.ast.attrs.of_decl(d);
        let mut all: Vec<AttrId> = self.ast.lists.iter(old).collect();
        all.extend_from_slice(attrs);
        let list = self.ast.lists.push(&all);
        if !self.ast.attrs.decl_attrs.refine(d, list) {
            let _ = self.ast.attrs.decl_attrs.insert(d, list);
        }
        self.ast.decls.set_has_attrs(d);
    }

    // ---------------------------------------------------------------------
    // Classes (spec/sema-class.md §8.1)
    // ---------------------------------------------------------------------

    /// An access specifier `public:` / `protected:` / `private:`
    /// ([class.access.spec]) in class `record`; the access is the
    /// declaration's `misc` access.
    pub fn access_spec(&mut self, record: DeclId, access: Access, loc: Loc) -> DeclId {
        let h = DeclHead::new(record, DeclName::EMPTY, loc).access(access);
        self.add(h, DeclKind::AccessSpec, None, Payload::None(0), None)
    }

    /// A friend declaration ([class.friend]) in class `record` at `loc`
    /// (the `friend` keyword). A befriended function or class is declared
    /// first (semantic context its namespace, lexical context `record`,
    /// `misc.friend()` set); a hidden friend enters its namespace's table
    /// with `LookupFlags::ADL_ONLY` (`FriendKind::Undeclared`). The class's
    /// `friends` list is written once at `}` ([`AstBuilder::set_friends`]).
    pub fn friend(&mut self, record: DeclId, loc: Loc, target: FriendTarget) -> DeclId {
        let h = DeclHead::new(record, DeclName::EMPTY, loc).access(Access::Public);
        match target {
            FriendTarget::Decl(d) => {
                self.add(h, DeclKind::Friend, None, Payload::None(d.raw()), None)
            }
            FriendTarget::Type { ty, owned_tag } => self.add(
                h,
                DeclKind::Friend,
                Some(ty),
                Payload::None(owned_tag.map_or(0, RawId::raw)),
                None,
            ),
        }
    }

    /// Records the befriended entities of class `record` (write-once at
    /// `}`; first declarations of functions and classes, used only by
    /// access checks, spec/sema-class.md §2.4). An empty slice records
    /// nothing.
    ///
    /// # Errors
    /// If already set.
    pub fn set_friends(&mut self, record: DeclId, friends: &[DeclId]) -> Result<(), AlreadySet> {
        if friends.is_empty() {
            return Ok(());
        }
        let list = self.ast.lists.push(friends);
        self.ast.decls.cold.friends.insert(record, list)
    }

    /// Sets the ctor-initializers of constructor `ctor` in initialization
    /// order, implicit ones included (write-once, with the body;
    /// spec/sema-class.md §8.2).
    ///
    /// # Errors
    /// If already set.
    ///
    /// # Panics
    /// If `ctor` is not a constructor.
    pub fn set_ctor_inits(&mut self, ctor: DeclId, inits: &[CtorInit]) -> Result<(), AlreadySet> {
        self.ast.decls.set_ctor_inits(ctor, inits, &mut self.ast.lists)
    }

    /// Records the virtual functions `method` overrides ([class.virtual]/2;
    /// write-once, at its declaration). An empty slice records nothing.
    ///
    /// # Errors
    /// If already set.
    pub fn set_overrides(
        &mut self,
        method: DeclId,
        overridden: &[DeclId],
    ) -> Result<(), AlreadySet> {
        if overridden.is_empty() {
            return Ok(());
        }
        let list = self.ast.lists.push(overridden);
        self.ast.decls.cold.overrides.insert(method, list)
    }

    /// The `LifetimeExtendedTemporary` of a temporary of type `ty` whose
    /// lifetime is extended by `extending` ([class.temporary]/6;
    /// `mangling_index` numbers the temporaries extended by `extending`).
    /// It lives in `extending`'s context but joins no member list or
    /// lookup table; its value is written later by the Oven.
    pub fn lifetime_extended_temporary(
        &mut self,
        extending: DeclId,
        mangling_index: u32,
        ty: QualType,
        loc: Loc,
    ) -> DeclId {
        let ctx = self.ast.decls.get(extending).ctx();
        self.ast.decls.push(
            NewDecl {
                kind: DeclKind::LifetimeExtendedTemporary,
                flags: DeclFlags::IMPLICIT,
                misc: DeclMisc::new(),
                name: DeclName::EMPTY,
                ctx,
                loc,
                ty: Some(ty),
            },
            Payload::Let(LetData { extending, mangling_index, value: None }),
        )
    }

    /// Declares an implicit special member or inheriting constructor of
    /// class `h.ctx` ([class.default.ctor], [class.copy.ctor],
    /// [class.copy.assign], [class.dtor], [namespace.udecl]/16), possibly
    /// after the class is complete (lazy declaration, spec/sema-class.md
    /// §4.1 and §8.2): it is appended to the member list and the class's
    /// lookup table. Parameters follow with [`AstBuilder::param`] /
    /// [`AstBuilder::set_params`].
    ///
    /// # Panics
    /// If `h.flags` lacks `IMPLICIT` or `kind` is not a constructor,
    /// method or destructor.
    pub fn append_implicit_member(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: QualType,
        spec: FnSpec,
    ) -> DeclId {
        assert!(h.flags.contains(DeclFlags::IMPLICIT), "implicit member without IMPLICIT");
        assert!(
            matches!(
                kind,
                DeclKind::CXXConstructor | DeclKind::CXXMethod | DeclKind::CXXDestructor
            ),
            "not a special member kind: {kind:?}"
        );
        self.function(h, kind, ty, spec, None)
    }

    /// Marks variable `v` as the named return value of its function
    /// ([class.copy.elision]/1.1; once, at its scope exit).
    pub fn set_nrvo(&mut self, v: DeclId) {
        self.ast.decls.mark_var(v, VarSpec::new().with_nrvo(true));
    }

    /// Drops the NRVO candidate of `return` statement `r` once its
    /// variable turned out not to be the named return value (clang
    /// `computeNRVO`, at the end of the function body).
    pub fn clear_return_nrvo(&mut self, r: StmtId) {
        self.ast.stmts.clear_return_nrvo(r);
    }

    // =====================================================================
    // Expressions
    // =====================================================================

    /// The dependence implied by type `ty` (§5.1).
    pub fn type_deps(&self, ty: QualType) -> ExprDeps {
        deps_of_type_flags(self.ast.types.flags(ty.ty()))
    }

    pub(crate) fn deps(&self, e: ExprId) -> ExprDeps {
        self.ast.exprs.get(e).deps()
    }

    fn vk(&self, e: ExprId) -> ValueCategory {
        self.ast.exprs.get(e).value_category()
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors the 10 header fields of ExprNode")]
    pub(crate) fn push_expr(
        &mut self,
        kind: ExprKind,
        vk: ValueCategory,
        ok: ObjectKind,
        flags: ExprFlags,
        deps: ExprDeps,
        aux: u8,
        ty: QualType,
        loc: Loc,
        a: u32,
        b: u32,
    ) -> ExprId {
        self.ast.exprs.push(ExprNode::new(kind, vk, ok, flags, deps, aux, ty, loc, a, b))
    }

    #[allow(clippy::too_many_arguments, reason = "header fields minus object kind and flags")]
    pub(crate) fn simple(
        &mut self,
        kind: ExprKind,
        vk: ValueCategory,
        deps: ExprDeps,
        aux: u8,
        ty: QualType,
        loc: Loc,
        a: u32,
        b: u32,
    ) -> ExprId {
        self.push_expr(kind, vk, ObjectKind::Ordinary, ExprFlags::NONE, deps, aux, ty, loc, a, b)
    }

    /// `p->~T` / `x.~T` naming the destructor of a scalar or dependent
    /// type `destroyed` ([expr.prim.id.dtor]/2, [expr.ref]/1):
    /// `CXXPseudoDestructorExpr`, a prvalue of the bound-member type
    /// whose only use is a call of type `void` ([expr.call]/5). `base`
    /// is the object (a pointer for `arrow`); `loc` is the destroyed
    /// type's name.
    pub fn pseudo_destructor(
        &mut self,
        base: ExprId,
        arrow: bool,
        destroyed: QualType,
        loc: Loc,
    ) -> ExprId {
        self.pseudo_destructor_q(base, arrow, false, destroyed, loc)
    }

    /// [`AstBuilder::pseudo_destructor`] recording whether the destructor name
    /// was qualified (`p->T::~T`, `this->B::~B` in a template): once the
    /// destroyed type is a class at instantiation, a qualified name calls
    /// the destructor non-virtually ([class.virtual]/16) and may name a
    /// base class's destructor ([expr.ref]/4).
    pub fn pseudo_destructor_q(
        &mut self,
        base: ExprId,
        arrow: bool,
        qualified: bool,
        destroyed: QualType,
        loc: Loc,
    ) -> ExprId {
        let deps = self.deps(base) | self.type_deps(destroyed);
        self.simple(
            ExprKind::CXXPseudoDestructorExpr,
            ValueCategory::PRValue,
            deps,
            u8::from(arrow) | (u8::from(qualified) << 1),
            QualType::BOUND_MEMBER,
            loc,
            base.raw(),
            destroyed.to_raw(),
        )
    }

    /// An integer literal of at most 64 bits ([lex.icon]): a prvalue,
    /// never dependent.
    pub fn int_literal(&mut self, ty: QualType, value: u64, radix: Radix, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        #[allow(clippy::cast_possible_truncation, reason = "split into two words")]
        let (lo, hi) = (value as u32, (value >> 32) as u32);
        self.simple(
            ExprKind::IntegerLiteral,
            ValueCategory::PRValue,
            deps,
            radix as u8,
            ty,
            loc,
            lo,
            hi,
        )
    }

    /// An integer literal wider than 64 bits (`__int128`, `_BitInt`), its
    /// value interned in the `ConstStore`.
    pub fn wide_int_literal(
        &mut self,
        ty: QualType,
        value: ConstValueId,
        radix: Radix,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        self.push_expr(
            ExprKind::IntegerLiteral,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            ExprFlags::WIDE,
            deps,
            radix as u8,
            ty,
            loc,
            value.raw(),
            0,
        )
    }

    /// A floating literal ([lex.fcon]), its value interned.
    pub fn float_literal(&mut self, ty: QualType, value: ConstValueId, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::FloatingLiteral,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            value.raw(),
            0,
        )
    }

    /// A GNU imaginary constant (clang `ImaginaryLiteral`, `2.0i`): the
    /// literal `sub` (of the element type) times *i*, of complex type `ty`.
    pub fn imaginary_literal(&mut self, sub: ExprId, ty: QualType, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::ImaginaryLiteral,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            sub.raw(),
            0,
        )
    }

    /// A character literal ([lex.ccon]) with code-unit value `value`.
    pub fn char_literal(&mut self, ty: QualType, kind: CharKind, value: u32, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::CharacterLiteral,
            ValueCategory::PRValue,
            deps,
            kind as u8,
            ty,
            loc,
            value,
            0,
        )
    }

    /// A string literal ([lex.string]): an lvalue ([expr.prim.literal]/1)
    /// of array type. `s` is already interned with
    /// `StringPool::intern(encoding, code_units)`.
    pub fn string_literal(
        &mut self,
        ty: QualType,
        kind: StringKind,
        s: StringLitId,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::StringLiteral,
            ValueCategory::LValue,
            deps,
            kind as u8,
            ty,
            loc,
            s.raw(),
            0,
        )
    }

    /// Records the last token of `e` where its children do not end it
    /// (clang `getEndLoc`, WP-1083): the last piece of a concatenated
    /// `StringLiteral` (a stringized `#x` piece lies in scratch space,
    /// which coverage maps as a nested expansion), the `}` of a scalar
    /// `CompoundLiteralExpr` (whose `InitListExpr` kiln folds away).
    pub fn record_end_loc(&mut self, e: ExprId, end: Loc) {
        let _ = self.ast.exprs.cold.end_loc.insert(e, end);
    }

    /// `true` / `false` ([lex.bool]).
    pub fn bool_literal(&mut self, ty: QualType, value: bool, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::CXXBoolLiteralExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            u32::from(value),
            0,
        )
    }

    /// `nullptr` ([lex.nullptr]).
    pub fn nullptr_literal(&mut self, ty: QualType, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(ExprKind::CXXNullPtrLiteralExpr, ValueCategory::PRValue, deps, 0, ty, loc, 0, 0)
    }

    /// GNU `&&label` (clang `AddrLabelExpr`): the address of the label
    /// declaration `label`, a `void *` prvalue `ty`, written at `loc`
    /// (the `&&`) with the label's name at `label_loc` (the end).
    pub fn addr_label(&mut self, label: DeclId, ty: QualType, loc: Loc, label_loc: Loc) -> ExprId {
        let e = self.simple(
            ExprKind::AddrLabelExpr,
            ValueCategory::PRValue,
            ExprDeps::NONE,
            0,
            ty,
            loc,
            label.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, label_loc).expect(FRESH_EXPR);
        e
    }

    /// GNU `__null`: `GNUNullExpr`, an integer null pointer constant of
    /// type `ty` (the signed integer type as wide as a pointer).
    pub fn gnu_null(&mut self, ty: QualType, loc: Loc) -> ExprId {
        self.simple(ExprKind::GNUNullExpr, ValueCategory::PRValue, ExprDeps::NONE, 0, ty, loc, 0, 0)
    }

    /// `__func__`, `__FUNCTION__` or `__PRETTY_FUNCTION__`
    /// ([dcl.fct.def.general]/8): `PredefinedExpr`, an lvalue of type `ty`
    /// (`const char[N]`, or the dependent type in a dependent context,
    /// where `name` is `None`); `name` is the `StringLiteral` holding the
    /// value (its child).
    pub fn predefined_expr(
        &mut self,
        kind: crate::expr::PredefinedKind,
        ty: QualType,
        name: Option<ExprId>,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::PredefinedExpr,
            ValueCategory::LValue,
            deps,
            kind as u8,
            ty,
            loc,
            name.map_or(0, ExprId::raw),
            0,
        )
    }

    /// A source-location builtin call `kind()` ([support.srcloc.cons];
    /// clang `SourceLocExpr`) of type `ty` written in context `ctx`, from
    /// `loc` to the `)` at `rparen`: a non-dependent prvalue (sema keeps a
    /// template pattern's context, spec/ast.md §5.8).
    pub fn source_loc_expr(
        &mut self,
        kind: crate::expr::SourceLocKind,
        ty: QualType,
        ctx: DeclId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        let e = self.simple(
            ExprKind::SourceLocExpr,
            ValueCategory::PRValue,
            deps,
            kind as u8,
            ty,
            loc,
            ctx.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// The value category of an id-expression naming `d`
    /// ([expr.prim.id.unqual]/3): lvalue for variables, functions, data
    /// members and template parameter objects; prvalue for enumerators and
    /// non-type template parameters ([temp.param]/8 — class-type ones name
    /// a `TemplateParamObject` once substituted).
    fn decl_ref_category(&self, d: DeclId) -> ValueCategory {
        match self.ast.decls.get(d).kind() {
            // [temp.param]/8: a class-type parameter names an object; a
            // reference parameter names its referent.
            DeclKind::NonTypeTemplateParm
                if self.ast.decls.get(d).ty().is_some_and(|t| {
                    self.ast.types.as_record(t).is_some() || self.ast.types.is_reference(t)
                }) =>
            {
                ValueCategory::LValue
            }
            DeclKind::EnumConstant | DeclKind::NonTypeTemplateParm => ValueCategory::PRValue,
            // A builtin function's name has the placeholder type
            // `<builtin fn type>` and is a prvalue (as clang).
            DeclKind::Function if self.ast.decls.builtin_id(d).is_some() => ValueCategory::PRValue,
            // A non-static member function is a prvalue ([expr.prim.id.qual]/5,
            // [expr.ref]/7.3); constructors, destructors and conversion
            // functions never are static.
            DeclKind::CXXMethod
                if self
                    .ast
                    .decls
                    .function(d)
                    .is_some_and(|f| f.spec.storage() != crate::decl::FnStorage::Static) =>
            {
                ValueCategory::PRValue
            }
            DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXConversion => {
                ValueCategory::PRValue
            }
            _ => ValueCategory::LValue,
        }
    }

    /// Dependence an id-expression gets from the declaration it names
    /// beyond its type ([temp.dep.constexpr]/2): a non-type template
    /// parameter is value-dependent; so is a potentially-constant variable
    /// with a value-dependent initializer. A declaration in a dependent
    /// context makes the reference instantiation-dependent.
    pub(crate) fn decl_deps(&self, d: DeclId) -> ExprDeps {
        let n = self.ast.decls.get(d);
        let mut deps = ExprDeps::NONE;
        if n.has(DeclFlags::DEPENDENT_CTX) {
            deps |= ExprDeps::INSTANTIATION;
        }
        // A function or template parameter pack names an unexpanded pack
        // ([temp.variadic]/5); its type is the pack expansion (a non-type
        // template parameter pack `int... N` keeps its pattern type).
        let nttp_pack = n.kind() == DeclKind::NonTypeTemplateParm
            && self.ast.template_parm(d).is_some_and(|i| i.pack);
        if nttp_pack
            || n.ty().is_some_and(|t| {
                matches!(self.ast.types.view(t.ty()), crate::types::TypeView::PackExpansion { .. })
            })
        {
            deps |= ExprDeps::UNEXPANDED_PACK | ExprDeps::INSTANTIATION;
        }
        match n.kind() {
            DeclKind::NonTypeTemplateParm => deps |= ExprDeps::VALUE_INST,
            // [temp.dep.constexpr]/2.3: a static member function that is
            // a dependent member of the current instantiation (clang's
            // `computeDependence` for `DeclRefExpr`): `if constexpr
            // (rank() > 0)` in a member of `extents` waits for
            // instantiation, where `rank()` reads `__rank_`.
            DeclKind::CXXMethod
                if self
                    .ast
                    .decls
                    .function(d)
                    .is_some_and(|f| f.spec.storage() == crate::decl::FnStorage::Static)
                    && n.ctx()
                        .is_some_and(|c| self.ast.decls.get(c).has(DeclFlags::DEPENDENT_CTX)) =>
            {
                deps |= ExprDeps::VALUE_INST;
            }
            DeclKind::Var => {
                if let Some(v) = self.ast.decls.var(d) {
                    // The `const` may come from a typedef (bsl's
                    // `typedef const bool Cbool; static Cbool k = ...;`).
                    // A reference is potentially-constant too
                    // ([expr.const]/3): `const int &i = k;` of a `k`
                    // initialized by `n` is value-dependent.
                    let potentially_constant = v.spec.is_constexpr()
                        || n.ty().is_some_and(|t| {
                            self.ast.types.effective_quals(t).is_const()
                                || self.ast.types.is_reference(t)
                        });
                    if potentially_constant
                        && v.init.is_some_and(|i| self.deps(i).contains(ExprDeps::VALUE))
                    {
                        deps |= ExprDeps::VALUE_INST;
                    }
                    // An array of unknown bound whose initializer is
                    // type-dependent: its bound is unknown until
                    // instantiation (clang's `computeDependence` for
                    // `DeclRefExpr`; `int a[] = {ts...}`).
                    if n.ty().is_some_and(|t| {
                        self.ast.types.canonical_kind(t) == crate::types::TypeKind::IncompleteArray
                    }) && v.init.is_some_and(|i| self.deps(i).contains(ExprDeps::TYPE))
                    {
                        deps |= ExprDeps::TYPE_VALUE_INST;
                    }
                }
                // [temp.dep.expr]/3: a static data member of the current
                // instantiation declared in its class as an array of
                // unknown bound, without an initializer there, is
                // type-dependent: its out-of-line definition gives the
                // bound (clang's `computeDependence` for `DeclRefExpr`;
                // `static const Info Ops[];` with `sizeof(Ops)` in a
                // member of the template).
                if n.has(DeclFlags::DEPENDENT_CTX)
                    && n.ctx().is_some_and(|c| self.ast.decls.get(c).kind() == DeclKind::CXXRecord)
                {
                    let first = self.ast.first(d);
                    let fty = self.ast.decls.get(first).ty();
                    if fty.is_some_and(|t| {
                        self.ast.types.canonical_kind(t) == crate::types::TypeKind::IncompleteArray
                    }) && self.ast.decls.var(first).is_some_and(|v| v.init.is_none())
                    {
                        deps |= ExprDeps::TYPE_VALUE_INST;
                    }
                }
            }
            _ => {}
        }
        deps
    }

    /// A `DeclRefExpr` naming `d` with type `ty` (the declared type with
    /// references dropped, [expr.type]/1). `extra` holds a qualifier,
    /// explicit template arguments or a found declaration (a using
    /// shadow); `flags` may carry `REFERS_TO_ENCLOSING` and
    /// `HAD_MULTIPLE_CANDIDATES`.
    pub fn decl_ref(
        &mut self,
        d: DeclId,
        ty: QualType,
        loc: Loc,
        extra: Option<DeclRefExtra>,
        non_odr_use: NonOdrUse,
        flags: ExprFlags,
    ) -> ExprId {
        let mut deps = self.type_deps(ty) | self.decl_deps(d);
        // An instantiation-dependent qualifier (`is_same<void,
        // void_t<decltype(e)>>::value`) makes the reference so (clang
        // `computeDependence(DeclRefExpr *)`).
        if let Some(q) = extra.and_then(|x| x.qualifier)
            && self.ast.types.nns_flags(q).contains(TypeFlags::INSTANTIATION_DEPENDENT)
        {
            deps |= ExprDeps::INSTANTIATION;
        }
        let vk = self.decl_ref_category(d);
        let b = extra.map_or(0, |x| self.ast.exprs.push_declref_extra(x));
        self.push_expr(
            ExprKind::DeclRefExpr,
            vk,
            ObjectKind::Ordinary,
            flags,
            deps,
            non_odr_use as u8,
            ty,
            loc,
            d.raw(),
            b,
        )
    }

    /// A class member access `base.m` / `base->m` ([expr.ref]); `base` is
    /// the object expression (for `->`, the pointer). Category
    /// ([expr.ref]/6): static data members and `->` accesses to fields
    /// and members of reference type are lvalues; `.` on another field
    /// takes the base's category, a prvalue
    /// base being materialized to an xvalue; enumerators and non-static
    /// member functions are prvalues. A bit-field gets object kind
    /// `BitField`.
    pub fn member(
        &mut self,
        base: ExprId,
        m: DeclId,
        ty: QualType,
        arrow: bool,
        loc: Loc,
        extra: Option<DeclRefExtra>,
    ) -> ExprId {
        let mn = *self.ast.decls.get(m);
        let base_vk = self.vk(base);
        let field_vk = if arrow {
            ValueCategory::LValue
        } else {
            match base_vk {
                ValueCategory::LValue => ValueCategory::LValue,
                ValueCategory::XValue | ValueCategory::PRValue => ValueCategory::XValue,
            }
        };
        let (vk, ok) = match mn.kind() {
            // A member of reference type is an lvalue ([expr.ref]/6.1).
            DeclKind::Field | DeclKind::IndirectField
                if mn.ty().is_some_and(|t| self.ast.types.is_reference(t)) =>
            {
                (ValueCategory::LValue, ObjectKind::Ordinary)
            }
            DeclKind::Field => {
                let bitfield = self.ast.decls.field(m).is_some_and(|f| f.bit_width.is_some());
                (field_vk, if bitfield { ObjectKind::BitField } else { ObjectKind::Ordinary })
            }
            DeclKind::IndirectField => (field_vk, ObjectKind::Ordinary),
            DeclKind::EnumConstant => (ValueCategory::PRValue, ObjectKind::Ordinary),
            k if k.is_function() => {
                let is_static = self
                    .ast
                    .decls
                    .function(m)
                    .is_some_and(|f| f.spec.storage() == crate::decl::FnStorage::Static);
                (
                    if is_static { ValueCategory::LValue } else { ValueCategory::PRValue },
                    ObjectKind::Ordinary,
                )
            }
            _ => (ValueCategory::LValue, ObjectKind::Ordinary),
        };
        let mut deps = self.deps(base) | self.type_deps(ty) | self.decl_deps(m);
        // A field of the current instantiation with a non-dependent type
        // is not type-dependent ([temp.dep.expr]/5); a bit-field with a
        // value-dependent width is (clang's `computeDependence` for
        // `MemberExpr`).
        if mn.kind() == DeclKind::Field {
            if mn.has(DeclFlags::DEPENDENT_CTX) && !self.ast.types.is_dependent(ty) {
                deps = deps.without(ExprDeps::TYPE);
            }
            let width = self.ast.decls.field(m).and_then(|f| f.bit_width);
            if width.is_some_and(|w| self.deps(w).contains(ExprDeps::VALUE)) {
                deps |= ExprDeps::TYPE;
            }
        }
        let flags = if arrow { ExprFlags::ARROW } else { ExprFlags::NONE };
        let e = self.push_expr(
            ExprKind::MemberExpr,
            vk,
            ok,
            flags,
            deps,
            NonOdrUse::None as u8,
            ty,
            loc,
            base.raw(),
            m.raw(),
        );
        if let Some(x) = extra {
            self.ast.exprs.cold.member_extra.insert(e, x).expect(FRESH_EXPR);
        }
        e
    }

    /// A built-in unary operator ([expr.unary.op], [expr.post.incr],
    /// [expr.pre.incr]): `*` and prefix `++`/`--` are lvalues, `__real` /
    /// `__imag` / `__extension__` keep the operand's category (`__imag` of
    /// a real operand is a prvalue), the rest are prvalues.
    pub fn unary(
        &mut self,
        op: UnaryOp,
        operand: ExprId,
        ty: QualType,
        loc: Loc,
        flags: ExprFlags,
    ) -> ExprId {
        let vk = match op {
            UnaryOp::Deref | UnaryOp::PreInc | UnaryOp::PreDec => ValueCategory::LValue,
            // `__imag` of a real operand is a prvalue (its zero).
            UnaryOp::Imag
                if self.ast.types.as_complex(self.ast.exprs.get(operand).ty()).is_none() =>
            {
                ValueCategory::PRValue
            }
            UnaryOp::Real | UnaryOp::Imag | UnaryOp::Extension => self.vk(operand),
            _ => ValueCategory::PRValue,
        };
        let deps = self.deps(operand) | self.type_deps(ty);
        // `__extension__` keeps the operand's object kind too (clang
        // `CreateBuiltinUnaryOp`): `&__extension__ s.bf` is ill-formed.
        let ok = if op == UnaryOp::Extension {
            self.ast.exprs.get(operand).object_kind()
        } else {
            ObjectKind::Ordinary
        };
        self.push_expr(
            ExprKind::UnaryOperator,
            vk,
            ok,
            flags,
            deps,
            op as u8,
            ty,
            loc,
            operand.raw(),
            0,
        )
    }

    /// A built-in binary operator ([expr.mul] .. [expr.comma]): `=` is an
    /// lvalue ([expr.assign]/1), `,` takes its right operand's category
    /// ([expr.comma]/1), the rest are prvalues.
    ///
    /// # Panics
    /// For compound assignments (use [`AstBuilder::compound_assign`]) and
    /// pointer-to-member operators (use [`AstBuilder::ptr_mem`]).
    pub fn binary(
        &mut self,
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
        ty: QualType,
        loc: Loc,
    ) -> ExprId {
        assert!(!op.is_compound_assignment(), "use compound_assign for {op:?}");
        assert!(!op.is_ptr_mem(), "use ptr_mem for {op:?}");
        let vk = match op {
            BinaryOp::Assign => ValueCategory::LValue,
            BinaryOp::Comma => self.vk(rhs),
            _ => ValueCategory::PRValue,
        };
        let deps = self.deps(lhs) | self.deps(rhs) | self.type_deps(ty);
        self.simple(ExprKind::BinaryOperator, vk, deps, op as u8, ty, loc, lhs.raw(), rhs.raw())
    }

    /// A `BinaryOperator` of type `ty` and category `vk` that is the
    /// syntactic form of a [`AstBuilder::pseudo_object`] (`a <=> b` of a
    /// synthesized three-way comparison; never evaluated).
    pub fn syntactic_binary(
        &mut self,
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
    ) -> ExprId {
        let deps = self.deps(lhs) | self.deps(rhs) | self.type_deps(ty);
        self.simple(ExprKind::BinaryOperator, vk, deps, op as u8, ty, loc, lhs.raw(), rhs.raw())
    }

    /// A pointer-to-member operator `.*` / `->*` ([expr.mptr.oper]/6: the
    /// category depends on the member kind and the object, so sema
    /// supplies it).
    ///
    /// # Panics
    /// If `op` is not `.*` or `->*`.
    pub fn ptr_mem(
        &mut self,
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
    ) -> ExprId {
        assert!(op.is_ptr_mem(), "not a pointer-to-member operator: {op:?}");
        let deps = self.deps(lhs) | self.deps(rhs) | self.type_deps(ty);
        self.simple(ExprKind::BinaryOperator, vk, deps, op as u8, ty, loc, lhs.raw(), rhs.raw())
    }

    /// A compound assignment `lhs @= rhs` ([expr.assign]/6): an lvalue,
    /// with the computation types (clang's `ComputeLHSTy` /
    /// `ComputeResultTy`) in its side record.
    ///
    /// # Panics
    /// If `op` is not a compound assignment.
    #[allow(clippy::too_many_arguments, reason = "the operator, both operands and three types")]
    pub fn compound_assign(
        &mut self,
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
        ty: QualType,
        lhs_ty: QualType,
        result_ty: QualType,
        loc: Loc,
    ) -> ExprId {
        assert!(op.is_compound_assignment(), "not a compound assignment: {op:?}");
        let deps = self.deps(lhs) | self.deps(rhs) | self.type_deps(ty);
        let side =
            self.ast.exprs.push_compound_assign(CompoundAssignExtra { rhs, lhs_ty, result_ty });
        self.simple(
            ExprKind::CompoundAssignOperator,
            ValueCategory::LValue,
            deps,
            op as u8,
            ty,
            loc,
            lhs.raw(),
            side,
        )
    }

    /// A conditional operator `c ? t : e` ([expr.cond]): a glvalue of the
    /// arms' shared category when both arms are glvalues of the same
    /// category ([expr.cond]/4, after sema's conversions), else a prvalue.
    pub fn conditional(
        &mut self,
        cond: ExprId,
        then: ExprId,
        els: ExprId,
        ty: QualType,
        loc: Loc,
    ) -> ExprId {
        let (tv, ev) = (self.vk(then), self.vk(els));
        // [expr.cond]/2.1: a throw-expression arm takes the other's category.
        let vk = match (self.is_throw(then), self.is_throw(els)) {
            (true, false) => ev,
            (false, true) => tv,
            _ if tv == ev && tv.is_glvalue() => tv,
            _ => ValueCategory::PRValue,
        };
        let deps = self.deps(cond) | self.deps(then) | self.deps(els) | self.type_deps(ty);
        let side = self.ast.exprs.push_cond_arms(then, els);
        self.simple(ExprKind::ConditionalOperator, vk, deps, 0, ty, loc, cond.raw(), side)
    }

    /// A subscript `base[idx]` ([expr.sub]/2): an xvalue if an operand is
    /// an xvalue (array xvalue), else an lvalue.
    pub fn subscript(&mut self, base: ExprId, idx: ExprId, ty: QualType, loc: Loc) -> ExprId {
        // [expr.sub]/2: the result is an xvalue if the array operand is
        // an xvalue (seen through its array-to-pointer conversion).
        let array_xvalue = |b: &Self, e: ExprId| {
            let n = b.ast.exprs.get(e);
            n.kind() == ExprKind::ImplicitCastExpr
                && b.ast.exprs.cast_kind(e) == CastKind::ArrayToPointerDecay
                && b.vk(ExprId::new(n.a())) == ValueCategory::XValue
        };
        let x = self.vk(base) == ValueCategory::XValue
            || self.vk(idx) == ValueCategory::XValue
            || array_xvalue(self, base)
            || array_xvalue(self, idx);
        let vk = if x { ValueCategory::XValue } else { ValueCategory::LValue };
        let deps = self.deps(base) | self.deps(idx) | self.type_deps(ty);
        self.simple(ExprKind::ArraySubscriptExpr, vk, deps, 0, ty, loc, base.raw(), idx.raw())
    }

    /// Records the `]` of a built-in subscript `e` (its clang `getEndLoc`,
    /// `RBracketLoc`), unless `e` is no `ArraySubscriptExpr` or has one.
    pub fn set_subscript_end(&mut self, e: ExprId, rsquare: Loc) {
        if self.ast.exprs.get(e).kind() == ExprKind::ArraySubscriptExpr {
            let _ = self.ast.exprs.cold.end_loc.insert(e, rsquare);
        }
    }

    /// `v[i]` on a vector `v` (a GNU/clang extension): the element, of
    /// `v`'s value category; a glvalue designates a vector component
    /// (clang `OK_VectorComponent`).
    pub fn vector_subscript(
        &mut self,
        base: ExprId,
        idx: ExprId,
        ty: QualType,
        loc: Loc,
    ) -> ExprId {
        let vk = self.vk(base);
        let ok = if vk == ValueCategory::PRValue {
            ObjectKind::Ordinary
        } else {
            ObjectKind::VectorComponent
        };
        let deps = self.deps(base) | self.deps(idx) | self.type_deps(ty);
        self.push_expr(
            ExprKind::ArraySubscriptExpr,
            vk,
            ok,
            ExprFlags::NONE,
            deps,
            0,
            ty,
            loc,
            base.raw(),
            idx.raw(),
        )
    }

    /// `_Generic(ctrl, T: e, ..., default: e)` (clang
    /// `GenericSelectionExpr`, a C11 extension in C++): `selected` is the
    /// index of the chosen association, `None` while the selection is
    /// result-dependent. As [`AstBuilder::choose_expr`], the chosen
    /// expression gives the type, value and object kind and the
    /// type/value dependence; every other bit comes from all operands
    /// (clang `computeDependence(GenericSelectionExpr*)`).
    pub fn generic_selection(
        &mut self,
        ctrl: GenericControl,
        assocs: &[GenericAssoc],
        selected: Option<usize>,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let (mut all, flags, mut words) = match ctrl {
            GenericControl::Expr(c) => {
                (self.deps(c), ExprFlags::NONE, vec![c.raw(), Loc::INVALID.to_raw()])
            }
            GenericControl::Type(q, l) => {
                (self.type_deps(q), ExprFlags::ARG_IS_TYPE, vec![q.raw(), l.to_raw()])
            }
        };
        for a in assocs {
            all |= self.deps(a.expr);
            if let Some(t) = a.ty {
                all |= self.type_deps(t);
            }
            words.extend([RawId::raw_opt(a.ty), a.ty_loc.to_raw(), a.expr.raw()]);
        }
        let list = self.ast.lists.push_raw(&words).raw();
        let picked = selected.and_then(|i| assocs.get(i)).map(|a| a.expr);
        let e = match picked {
            None => self.push_expr(
                ExprKind::GenericSelectionExpr,
                ValueCategory::PRValue,
                ObjectKind::Ordinary,
                flags,
                all | ExprDeps::TYPE_VALUE_INST,
                0,
                QualType::DEPENDENT,
                loc,
                list,
                0,
            ),
            Some(active) => {
                let n = *self.ast.exprs.get(active);
                let tv = ExprDeps::TYPE.with(ExprDeps::VALUE);
                let d = n.deps();
                let deps = d.without(d.without(tv)) | all.without(tv);
                self.push_expr(
                    ExprKind::GenericSelectionExpr,
                    n.value_category(),
                    n.object_kind(),
                    flags,
                    deps,
                    0,
                    n.ty(),
                    loc,
                    list,
                    active.raw(),
                )
            }
        };
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// `__builtin_choose_expr(cond, lhs, rhs)` (clang `ChooseExpr`):
    /// `chosen` is `Some(true)` when the constant condition picked `lhs`,
    /// `Some(false)` for `rhs`, `None` while the condition is dependent.
    /// The chosen operand gives the type, value and object kind and the
    /// type/value dependence (clang `computeDependence(ChooseExpr*)`).
    pub fn choose_expr(
        &mut self,
        cond: ExprId,
        lhs: ExprId,
        rhs: ExprId,
        chosen: Option<bool>,
        loc: Loc,
    ) -> ExprId {
        let all = self.deps(cond) | self.deps(lhs) | self.deps(rhs);
        let side = self.ast.exprs.push_cond_arms(lhs, rhs);
        let Some(pick) = chosen else {
            return self.simple(
                ExprKind::ChooseExpr,
                ValueCategory::PRValue,
                all | ExprDeps::TYPE_VALUE_INST,
                0,
                QualType::DEPENDENT,
                loc,
                cond.raw(),
                side,
            );
        };
        let active = if pick { lhs } else { rhs };
        let n = *self.ast.exprs.get(active);
        let tv = ExprDeps::TYPE.with(ExprDeps::VALUE);
        // The active operand's type/value bits, every other bit of all three.
        let d = n.deps();
        let deps = d.without(d.without(tv)) | all.without(tv);
        let aux = if pick { 1 } else { 2 };
        self.push_expr(
            ExprKind::ChooseExpr,
            n.value_category(),
            n.object_kind(),
            ExprFlags::NONE,
            deps,
            aux,
            n.ty(),
            loc,
            cond.raw(),
            side,
        )
    }

    /// An ext-vector element access `base.accessor` or `base->accessor`
    /// (clang `ExtVectorElementExpr`): of type `ty` and value category
    /// `vk`; a glvalue designates a vector component. `base` is the
    /// vector, or for `->` the pointer to it. `loc` is the accessor.
    pub fn ext_vector_element(
        &mut self,
        base: ExprId,
        accessor: Symbol,
        ty: QualType,
        vk: ValueCategory,
        arrow: bool,
        loc: Loc,
    ) -> ExprId {
        let ok = if vk == ValueCategory::PRValue {
            ObjectKind::Ordinary
        } else {
            ObjectKind::VectorComponent
        };
        let deps = self.deps(base) | self.type_deps(ty);
        let flags = if arrow { ExprFlags::ARROW } else { ExprFlags::NONE };
        self.push_expr(
            ExprKind::ExtVectorElementExpr,
            vk,
            ok,
            flags,
            deps,
            0,
            ty,
            loc,
            base.raw(),
            accessor.index(),
        )
    }

    /// `__builtin_convertvector(src, T)` (clang `ConvertVectorExpr`): a
    /// prvalue of the written vector type `ty` whose operand `src` is kept
    /// unconverted, as clang does (codegen and the Oven read it).
    pub fn convert_vector(&mut self, src: ExprId, ty: QualType, loc: Loc, rparen: Loc) -> ExprId {
        let deps = self.deps(src) | self.type_deps(ty);
        let e = self.push_expr(
            ExprKind::ConvertVectorExpr,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            deps,
            0,
            ty,
            loc,
            src.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// A parenthesized expression ([expr.prim.paren]): the type, category,
    /// object kind and dependence of `sub`.
    pub fn paren(&mut self, sub: ExprId, lparen: Loc, rparen: Loc) -> ExprId {
        let n = *self.ast.exprs.get(sub);
        self.push_expr(
            ExprKind::ParenExpr,
            n.value_category(),
            n.object_kind(),
            ExprFlags::NONE,
            n.deps(),
            0,
            n.ty(),
            lparen,
            sub.raw(),
            rparen.to_raw(),
        )
    }

    /// A function call ([expr.call]); `vk` follows the return type
    /// ([expr.call]/13: lvalue reference → lvalue, rvalue reference to
    /// object → xvalue, else prvalue), which sema knows. `flags` may carry
    /// `ADL`.
    #[allow(
        clippy::too_many_arguments,
        reason = "callee, arguments, type, category and two locations"
    )]
    pub fn call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
        rparen: Loc,
        flags: ExprFlags,
    ) -> ExprId {
        let mut deps = self.deps(callee) | self.type_deps(ty);
        for &a in args {
            deps |= self.deps(a);
        }
        let list = self.ast.lists.push(args);
        let e = self.push_expr(
            ExprKind::CallExpr,
            vk,
            ObjectKind::Ordinary,
            flags,
            deps,
            0,
            ty,
            loc,
            callee.raw(),
            list.raw(),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// A call of an overloaded operator function (`CXXOperatorCallExpr`,
    /// spec/sema-overload.md §8): `callee` the decayed reference to the
    /// operator function, `args` its operands in parameter order (the
    /// object first for a member). `loc` is the operator token and `end`
    /// the closing `)`/`]` of a call or subscript (else invalid).
    #[allow(clippy::too_many_arguments, reason = "the call's parts")]
    pub fn operator_call(
        &mut self,
        op: crate::names::OverloadedOperator,
        callee: ExprId,
        args: &[ExprId],
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let mut deps = self.deps(callee) | self.type_deps(ty);
        for &a in args {
            deps |= self.deps(a);
        }
        let list = self.ast.lists.push(args);
        let e = self.push_expr(
            ExprKind::CXXOperatorCallExpr,
            vk,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            deps,
            op as u8,
            ty,
            loc,
            callee.raw(),
            list.raw(),
        );
        if end.is_valid() {
            self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        }
        e
    }

    /// The callee of an operator call (clang `CreateFunctionRefExpr`): an
    /// lvalue `DeclRefExpr` of the operator function `d` of type `ty`
    /// (a member function too), decayed to a pointer.
    pub fn operator_callee(
        &mut self,
        d: DeclId,
        ty: QualType,
        loc: Loc,
        extra: Option<DeclRefExtra>,
    ) -> ExprId {
        let deps = self.type_deps(ty) | self.decl_deps(d);
        let b = extra.map_or(0, |x| self.ast.exprs.push_declref_extra(x));
        let dre = self.push_expr(
            ExprKind::DeclRefExpr,
            ValueCategory::LValue,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            deps,
            NonOdrUse::None as u8,
            ty,
            loc,
            d.raw(),
            b,
        );
        let pty = self.ast.types.pointer_to(ty);
        self.cast(
            ExprKind::ImplicitCastExpr,
            CastKind::FunctionToPointerDecay,
            dre,
            pty,
            ValueCategory::PRValue,
            None,
            loc,
            ExprFlags::NONE,
        )
    }

    /// A comparison rewritten to `==` or `<=>` ([over.match.oper]/3.4,
    /// `CXXRewrittenBinaryOperator`): `semantic` is the expression it
    /// stands for, `loc` the written operator; `op` and the operands
    /// `l`, `r` are the expression as written (kept for template
    /// instantiation, not dumped).
    pub fn rewritten_binary(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        semantic: ExprId,
        loc: Loc,
    ) -> ExprId {
        let deps = self.deps(semantic);
        let ty = self.ast.exprs.get(semantic).ty();
        let vk = self.vk(semantic);
        let list = self.ast.lists.push(&[l, r]);
        self.push_expr(
            ExprKind::CXXRewrittenBinaryOperator,
            vk,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            deps,
            op as u8,
            ty,
            loc,
            semantic.raw(),
            list.raw(),
        )
    }

    /// A cast of `kind` (`ImplicitCastExpr` or an explicit cast kind) with
    /// cast kind `ck` ([expr.cast], [expr.static.cast], \[conv\]).
    /// Dependence ([temp.dep.expr]/3, [temp.dep.constexpr]/2): type-
    /// dependent iff the target type is; value-dependent if the target
    /// type is dependent or the operand value-dependent. `path` is the
    /// base path of derived-to-base / base-to-derived casts; `flags` may
    /// carry `PART_OF_EXPLICIT_CAST`.
    ///
    /// # Panics
    /// If `kind` is not a cast kind.
    #[allow(clippy::too_many_arguments, reason = "the cast header fields")]
    pub fn cast(
        &mut self,
        kind: ExprKind,
        ck: CastKind,
        sub: ExprId,
        ty: QualType,
        vk: ValueCategory,
        path: Option<ListId>,
        loc: Loc,
        flags: ExprFlags,
    ) -> ExprId {
        assert!(kind.is_cast(), "not a cast kind: {kind:?}");
        let deps = self.type_deps(ty) | self.deps(sub).without(ExprDeps::TYPE);
        let b = path.map_or(0, RawId::raw);
        self.push_expr(kind, vk, ObjectKind::Ordinary, flags, deps, ck as u8, ty, loc, sub.raw(), b)
    }

    /// An `ImplicitCastExpr` (\[conv\]) at the operand's location.
    pub fn implicit_cast(
        &mut self,
        ck: CastKind,
        sub: ExprId,
        ty: QualType,
        vk: ValueCategory,
    ) -> ExprId {
        let loc = self.ast.exprs.get(sub).loc();
        self.cast(ExprKind::ImplicitCastExpr, ck, sub, ty, vk, None, loc, ExprFlags::NONE)
    }

    /// `sizeof(T)` / `alignof(T)` ([expr.sizeof], [expr.alignof]): value-
    /// dependent if `arg` is dependent ([temp.dep.constexpr]/3); the result
    /// type (`std::size_t`) is never dependent.
    pub fn trait_of_type(
        &mut self,
        kind: TraitKind,
        arg: QualType,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let a = self.type_deps(arg);
        let mut deps = self.type_deps(ty) | a.without(ExprDeps::TYPE);
        if a.contains(ExprDeps::TYPE) {
            deps |= ExprDeps::VALUE_INST;
        }
        let e = self.push_expr(
            ExprKind::UnaryExprOrTypeTraitExpr,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            ExprFlags::ARG_IS_TYPE,
            deps,
            kind as u8,
            ty,
            loc,
            arg.to_raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// `sizeof expr` / `alignof expr`: value-dependent if `arg` is
    /// type-dependent ([temp.dep.constexpr]/3; its value does not matter).
    pub fn trait_of_expr(
        &mut self,
        kind: TraitKind,
        arg: ExprId,
        ty: QualType,
        loc: Loc,
    ) -> ExprId {
        let a = self.deps(arg);
        let mut deps = self.type_deps(ty) | a.without(ExprDeps::TYPE).without(ExprDeps::VALUE);
        if a.contains(ExprDeps::TYPE) {
            deps |= ExprDeps::VALUE_INST;
        }
        if matches!(kind, TraitKind::AlignOf | TraitKind::PreferredAlignOf)
            && self.names_dependently_aligned_decl(arg)
        {
            deps |= ExprDeps::VALUE_INST;
        }
        self.simple(
            ExprKind::UnaryExprOrTypeTraitExpr,
            ValueCategory::PRValue,
            deps,
            kind as u8,
            ty,
            loc,
            arg.raw(),
            0,
        )
    }

    /// `alignof e` is value-dependent when `e` (under parentheses) names
    /// a variable or member whose `alignas` operand is dependent (clang's
    /// `computeDependence(UnaryExprOrTypeTraitExpr *)`, [temp.dep.constexpr]/2).
    fn names_dependently_aligned_decl(&self, arg: ExprId) -> bool {
        let ast = &*self.ast;
        let mut e = arg;
        while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = ast.exprs.operand(e);
        }
        if !matches!(ast.exprs.get(e).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            return false;
        }
        let d = ast.exprs.decl(e);
        if !ast.decls.get(d).has(DeclFlags::HAS_ATTRS) {
            return false;
        }
        // clang `AlignedAttr::isAlignmentDependent`: a dependent type-id or
        // a value-dependent expression.
        let dependent = ExprDeps::TYPE.with(ExprDeps::VALUE);
        ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)).any(|id| {
            let n = ast.attrs.get(id);
            n.kind == crate::attr::AttrKind::ALIGNED
                && (QualType::from_raw(n.b)
                    .is_some_and(|t| self.type_deps(t).intersects(dependent))
                    || ExprId::from_raw(n.a).is_some_and(|x| self.deps(x).intersects(dependent)))
        })
    }

    /// `__builtin_va_arg(list, T)` (clang's `VAArgExpr`): a prvalue of
    /// `ty` (the non-reference form of `written`, clang's
    /// `getNonLValueExprType`) reading the next variadic argument through
    /// the `va_list` lvalue `list`; `ty_loc` is where `written` begins
    /// and `rparen` is the closing `)`. Dependence is `written`'s plus `list`'s without its type
    /// dependence (clang's `computeDependence(VAArgExpr *)`).
    pub fn va_arg_expr(
        &mut self,
        list: ExprId,
        written: QualType,
        ty: QualType,
        ty_loc: Loc,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let deps = self.type_deps(written) | self.deps(list).without(ExprDeps::TYPE);
        let e = self.simple(
            ExprKind::VAArgExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            list.raw(),
            written.to_raw(),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        self.ast.exprs.cold.trait_rhs_loc.insert(e, ty_loc).expect(FRESH_EXPR);
        e
    }

    /// `noexcept(operand)` ([expr.unary.noexcept]): a `bool` prvalue whose
    /// `value` sema computed from the unevaluated `operand`; `rparen` is
    /// the closing `)`. Value-dependent if `operand` is type- or
    /// value-dependent ([temp.dep.constexpr]/3).
    pub fn noexcept_expr(
        &mut self,
        operand: ExprId,
        value: bool,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let a = self.deps(operand);
        let mut deps = self.type_deps(ty) | a.without(ExprDeps::TYPE).without(ExprDeps::VALUE);
        if a.contains(ExprDeps::TYPE) || a.contains(ExprDeps::VALUE) {
            deps |= ExprDeps::VALUE_INST;
        }
        let e = self.simple(
            ExprKind::CXXNoexceptExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            operand.raw(),
            u32::from(value),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// `__trait(T...)` ([meta.unary], spec/sema-templates.md §9.1): a
    /// `bool` prvalue whose `value` sema computed from the unevaluated
    /// type operands `args`. Value-dependent if any operand is dependent
    /// ([temp.dep.constexpr]/3); `rparen` is the closing `)`.
    #[allow(clippy::too_many_arguments, reason = "one per field of the node")]
    pub fn type_trait_expr(
        &mut self,
        trait_: TypeTrait,
        args: &[QualType],
        rhs_loc: Option<Loc>,
        value: bool,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &a in args {
            let d = self.type_deps(a);
            deps |= d.without(ExprDeps::TYPE);
            if d.contains(ExprDeps::TYPE) {
                deps |= ExprDeps::VALUE_INST;
            }
        }
        let list = self.ast.lists.push(args);
        let e = self.simple(
            ExprKind::TypeTraitExpr,
            ValueCategory::PRValue,
            deps,
            trait_ as u8,
            ty,
            loc,
            list.raw(),
            u32::from(value),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        if let Some(l) = rhs_loc {
            self.ast.exprs.cold.trait_rhs_loc.insert(e, l).expect(FRESH_EXPR);
        }
        e
    }

    /// `^^T` / `^^entity` (P2996 [expr.reflect]): a `std::meta::info`
    /// prvalue. `aux` is the operand's [`Reflectee::tag`], `a` its
    /// [`Reflectee::payload`], `b` the [`crate::ids::ReflectionId`] index
    /// (0 while the operand is dependent: the expression is then
    /// value-dependent; also 0 when the reflection table is full, which
    /// the caller diagnoses: a non-dependent expression whose
    /// `reflect_value` is `None`). A type is interned as
    /// [`crate::Ast::reflection_type`] keys it; a declaration of a
    /// dependent context (a member of a class template, a local of a
    /// function template) is instantiation-dependent, so substitution
    /// remaps it.
    ///
    /// [`Reflectee::tag`]: crate::reflect::Reflectee::tag
    /// [`Reflectee::payload`]: crate::reflect::Reflectee::payload
    pub fn reflect_expr(
        &mut self,
        operand: crate::reflect::Reflectee,
        ty: QualType,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        use crate::reflect::Reflectee;
        let mut deps = ExprDeps::NONE;
        let mut operand = operand;
        match operand {
            Reflectee::Type(t) => {
                // An instantiation-dependent type (`ND<T>` with
                // `template <class T> using ND = int;`) is not a dependent
                // type, but its reflection still names the alias
                // specialization of the template's own parameters, so the
                // value differs per specialization: value-dependent, so
                // substitution rebuilds it ([temp.dep.constexpr]).
                let d = self.type_deps(t);
                deps |= d.without(ExprDeps::TYPE);
                if d.intersects(ExprDeps::TYPE | ExprDeps::INSTANTIATION) {
                    deps |= ExprDeps::VALUE_INST;
                }
                if !deps.contains(ExprDeps::VALUE_INST) {
                    operand = Reflectee::Type(self.ast.reflection_type(t));
                }
            }
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                if self.reflectee_in_dependent_ctx(d) {
                    deps |= ExprDeps::VALUE_INST;
                }
            }
            Reflectee::Template(n) => {
                let flags = self.ast.types.template_name_flags(n);
                if flags.intersects(TypeFlags::DEPENDENT_INSTANTIATION)
                    || self
                        .ast
                        .types
                        .template_decl(n)
                        .is_some_and(|d| self.reflectee_in_dependent_ctx(d))
                {
                    deps |= ExprDeps::VALUE_INST;
                }
            }
            _ => {}
        }
        let b = if deps.contains(ExprDeps::VALUE_INST) {
            0
        } else {
            self.ast.reflections.intern(operand).map_or(0, crate::ids::ReflectionId::index)
        };
        let (aux, a) = (operand.tag(), operand.payload());
        let e =
            self.simple(ExprKind::CXXReflectExpr, ValueCategory::PRValue, deps, aux, ty, loc, a, b);
        self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        e
    }

    /// Whether a reflection of declaration `d` is value-dependent because
    /// `d` sits in a dependent context. A friend class (template) first
    /// declared inside a class template is a member of the enclosing
    /// namespace ([namespace.memdef]/3): its `DEPENDENT_CTX` reflects
    /// only the lexical context, so `^^ns::list` stays independent even
    /// when `list`'s first declaration is `template <class, class>
    /// friend class list;` in some `Node<V>` (spec/reflection-repr.md §2).
    fn reflectee_in_dependent_ctx(&self, d: DeclId) -> bool {
        let n = self.ast.decls.get(d);
        if !n.has(DeclFlags::DEPENDENT_CTX) {
            return false;
        }
        let friend_class = n.misc().friend() != FriendKind::None
            && matches!(n.kind(), DeclKind::ClassTemplate | DeclKind::CXXRecord);
        let ns_member = n.ctx().is_some_and(|c| {
            matches!(self.ast.decls.get(c).kind(), DeclKind::Namespace | DeclKind::TranslationUnit)
        });
        !(friend_class && ns_member)
    }

    /// `^^nns name` with a dependent `nns` (P2996 [expr.reflect]/7,
    /// spec/reflection-repr.md §2): a value-dependent `CXXReflectExpr`
    /// over `name`, the `DependentScopeDeclRefExpr` substitution resolves.
    pub fn dependent_reflect_expr(
        &mut self,
        name: ExprId,
        ty: QualType,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let mut deps = ExprDeps::VALUE_INST;
        for d in [ExprDeps::UNEXPANDED_PACK, ExprDeps::ERROR] {
            if self.ast.exprs.get(name).deps().contains(d) {
                deps |= d;
            }
        }
        let e = self.simple(
            ExprKind::CXXReflectExpr,
            ValueCategory::PRValue,
            deps,
            crate::reflect::Reflectee::DEPENDENT_NAME_TAG,
            ty,
            loc,
            name.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        e
    }

    /// A value-dependent splice `[: operand :]` in an expression position
    /// (spec/reflection.md §5; template patterns only): `a` is the operand,
    /// `b` the object expression of a member splice `base.[: m :]` /
    /// `base->[: m :]` (0 otherwise), `aux` bit 0 the `->`. A splice
    /// whose operand is not dependent is the expression it designates.
    pub fn splice_expr(
        &mut self,
        operand: ExprId,
        base: Option<(ExprId, bool)>,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        self.splice_node(operand, base, false, loc, end)
    }

    fn splice_node(
        &mut self,
        operand: ExprId,
        base: Option<(ExprId, bool)>,
        unexpanded_args: bool,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let mut deps = ExprDeps::TYPE_VALUE_INST;
        if unexpanded_args {
            deps |= ExprDeps::UNEXPANDED_PACK;
        }
        let unexpanded =
            |s: &Self, e: ExprId| s.ast.exprs.get(e).deps().contains(ExprDeps::UNEXPANDED_PACK);
        if unexpanded(self, operand) || base.is_some_and(|(b, _)| unexpanded(self, b)) {
            deps |= ExprDeps::UNEXPANDED_PACK;
        }
        let aux = u8::from(base.is_some_and(|(_, arrow)| arrow));
        let b = base.map_or(0, |(b, _)| b.raw());
        let e = self.simple(
            ExprKind::CXXSpliceExpr,
            ValueCategory::LValue,
            deps,
            aux,
            QualType::DEPENDENT,
            loc,
            operand.raw(),
            b,
        );
        self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        e
    }

    /// A dependent splice template-id `template [: operand :] < args >`
    /// in an expression position (P2996 [expr.prim.splice]): a
    /// [`Self::splice_expr`] that also keeps the template arguments as
    /// written, substituted with the operand.
    pub fn splice_template_expr(
        &mut self,
        operand: ExprId,
        args: ArgListId,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let unexpanded = self.ast.types.arg_list_flags(args).contains(TypeFlags::UNEXPANDED_PACK);
        let e = self.splice_node(operand, None, unexpanded, loc, end);
        self.ast.exprs.cold.splice_args.insert(e, args).expect(FRESH_EXPR);
        e
    }

    /// `__array_rank(T)` / `__array_extent(T, dim)` (§9.4): a `size_t`
    /// prvalue. Value-dependent if `arg` or `dim` is.
    #[allow(clippy::too_many_arguments, reason = "one per field of the node")]
    pub fn array_type_trait_expr(
        &mut self,
        trait_: TypeTrait,
        arg: QualType,
        dim: Option<ExprId>,
        value: u64,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let a = self.type_deps(arg);
        let mut deps = self.type_deps(ty) | a.without(ExprDeps::TYPE);
        if a.contains(ExprDeps::TYPE) {
            deps |= ExprDeps::VALUE_INST;
        }
        if let Some(d) = dim {
            let dd = self.deps(d);
            deps |= dd.without(ExprDeps::TYPE);
            if dd.contains(ExprDeps::TYPE) {
                deps |= ExprDeps::VALUE_INST;
            }
        }
        let e = self.simple(
            ExprKind::ArrayTypeTraitExpr,
            ValueCategory::PRValue,
            deps,
            trait_ as u8,
            ty,
            loc,
            arg.to_raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        self.ast
            .exprs
            .cold
            .array_trait
            .insert(e, ArrayTraitExtra { dim, value })
            .expect(FRESH_EXPR);
        e
    }

    /// `__is_lvalue_expr(e)` / `__is_rvalue_expr(e)`: a `bool` prvalue;
    /// value-dependent if `operand` is type-dependent.
    pub fn expression_trait_expr(
        &mut self,
        trait_: TypeTrait,
        operand: ExprId,
        value: bool,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let a = self.deps(operand);
        let mut deps = self.type_deps(ty) | a.without(ExprDeps::TYPE).without(ExprDeps::VALUE);
        if a.contains(ExprDeps::TYPE) {
            deps |= ExprDeps::VALUE_INST;
        }
        let e = self.simple(
            ExprKind::ExpressionTraitExpr,
            ValueCategory::PRValue,
            deps,
            trait_ as u8,
            ty,
            loc,
            operand.raw(),
            u32::from(value),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// A semantic `InitListExpr` ([dcl.init.list], §5.6): a prvalue whose
    /// explicit initializers are `inits`; `array_filler` initializes the
    /// remaining elements; `union_field` is the initialized union member.
    pub fn init_list(
        &mut self,
        ty: QualType,
        inits: &[ExprId],
        array_filler: Option<ExprId>,
        union_field: Option<DeclId>,
        lbrace: Loc,
        rbrace: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &i in inits.iter().chain(array_filler.as_ref()) {
            deps |= self.deps(i);
        }
        let list = self.ast.lists.push(inits);
        let side =
            self.ast.exprs.push_init_list(InitListExtra { array_filler, union_field, rbrace });
        self.simple(
            ExprKind::InitListExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            lbrace,
            list.raw(),
            side,
        )
    }

    /// An initializer-clause `designators = init` of a dependent
    /// braced-init-list (§5.6; clang `ActOnDesignatedInitializer`):
    /// `DesignatedInitExpr` of type `void`, with the value category of
    /// `init`. `loc` is its start (the first designator).
    pub fn designated_init(
        &mut self,
        designators: &[crate::expr::Designator],
        init: ExprId,
        loc: Loc,
    ) -> ExprId {
        let mut deps = self.deps(init);
        for d in designators {
            match *d {
                crate::expr::Designator::Index { index, .. } => deps |= self.deps(index),
                crate::expr::Designator::Range { first, last, .. } => {
                    deps |= self.deps(first) | self.deps(last);
                }
                crate::expr::Designator::Field { .. } => {}
            }
        }
        let vk = self.vk(init);
        let e = self.simple(
            ExprKind::DesignatedInitExpr,
            vk,
            deps,
            0,
            QualType::VOID,
            loc,
            init.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.designators.insert(e, designators.into());
        e
    }

    /// An `ImplicitValueInitExpr` ([dcl.init.general]/9: value-initialize
    /// an element with no explicit initializer).
    pub fn implicit_value_init(&mut self, ty: QualType) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::ImplicitValueInitExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            Loc::INVALID,
            0,
            0,
        )
    }

    /// A `CXXNewExpr` ([expr.new], §5.9): a prvalue of pointer type `ty`
    /// starting at `loc` (`::` or `new`); `extra` holds the allocated type,
    /// the converted array size, initializer and placement arguments and the
    /// allocation / deallocation functions; `end` is the last token.
    pub fn cxx_new(
        &mut self,
        ty: QualType,
        flags: ExprFlags,
        style: NewInitStyle,
        extra: NewExtra,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        // [temp.dep.expr]/3-4: only the allocated type makes a
        // new-expression type-dependent; a type-dependent array bound,
        // initializer or placement argument makes it value-dependent
        // (clang's `computeDependence(CXXNewExpr *)` and
        // `turnTypeToValueDependence`).
        let operand = |d: ExprDeps| {
            if d.contains(ExprDeps::TYPE) {
                d.without(ExprDeps::TYPE).with(ExprDeps::VALUE)
            } else {
                d
            }
        };
        let mut deps = self.type_deps(ty) | self.type_deps(extra.allocated_ty);
        for e in [extra.array_size, extra.init].into_iter().flatten() {
            deps |= operand(self.deps(e));
        }
        if let Some(p) = extra.placement {
            let args: Vec<ExprId> = self.ast.lists.iter::<ExprId>(p).collect();
            for e in args {
                deps |= operand(self.deps(e));
            }
        }
        let side = self.ast.exprs.push_new_extra(extra);
        let e = self.push_expr(
            ExprKind::CXXNewExpr,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            flags,
            deps,
            style as u8,
            ty,
            loc,
            side,
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        e
    }

    /// A `CXXDeleteExpr` ([expr.delete], §5.9): a `void` prvalue deleting
    /// `operand` with `operator_delete`; `sized`/`aligned` say whether the
    /// size and the alignment are passed to it.
    #[allow(clippy::too_many_arguments, reason = "one per payload field")]
    pub fn cxx_delete(
        &mut self,
        operand: ExprId,
        operator_delete: Option<DeclId>,
        flags: ExprFlags,
        sized: bool,
        aligned: bool,
        ty: QualType,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty) | self.deps(operand);
        let aux = u8::from(sized) | (u8::from(aligned) << 1);
        self.push_expr(
            ExprKind::CXXDeleteExpr,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            flags,
            deps,
            aux,
            ty,
            loc,
            operand.raw(),
            operator_delete.map_or(0, DeclId::raw),
        )
    }

    /// A `ConstantExpr` wrapping a constant expression ([expr.const]) with
    /// its value when known (§5.5; also settable later with
    /// [`crate::expr::ExprArena::set_constant_value`]).
    ///
    /// # Panics
    /// If a value is given for a value-dependent `sub`.
    pub fn constant_expr(&mut self, sub: ExprId, value: Option<ConstValueId>) -> ExprId {
        let n = *self.ast.exprs.get(sub);
        assert!(
            value.is_none() || !n.is_value_dependent(),
            "value-dependent ConstantExpr has no value"
        );
        self.push_expr(
            ExprKind::ConstantExpr,
            n.value_category(),
            n.object_kind(),
            ExprFlags::NONE,
            n.deps(),
            0,
            n.ty(),
            n.loc(),
            sub.raw(),
            value.map_or(0, RawId::raw),
        )
    }

    /// A `RecoveryExpr` for an invalid expression, keeping its valid
    /// `children` (§5.3). Without a known type it has the `Dependent`
    /// placeholder type. Always contains an error, is value- and
    /// instantiation-dependent (so constant evaluation and instantiation
    /// skip it) and type-dependent if its type is.
    pub fn recovery(
        &mut self,
        ty: Option<QualType>,
        vk: ValueCategory,
        children: &[ExprId],
        loc: Loc,
    ) -> ExprId {
        let ty = ty.unwrap_or_else(|| QualType::unqual(TypeId::new(BUILTIN_DEPENDENT)));
        let mut deps = self.type_deps(ty) | ExprDeps::ERROR | ExprDeps::VALUE_INST;
        for &c in children {
            deps |= self.deps(c);
        }
        let list = self.ast.lists.push(children);
        self.simple(ExprKind::RecoveryExpr, vk, deps, 0, ty, loc, list.raw(), 0)
    }

    // ----- WP-154: atomic builtins --------------------------------------

    /// An `AtomicExpr` (clang's `AtomicExpr`): a `__c11_atomic_*` /
    /// `__atomic_*` operation `op` (its builtin id, see
    /// [`crate::decl::builtin::atomic`]) of the converted operands `subs`
    /// in clang's order (`ptr`, `order`, `val1`, `order_fail`, `val2`,
    /// `weak`, as the form has them); `loc` is the builtin name's
    /// location, `rparen` the closing `)`.
    pub fn atomic_expr(
        &mut self,
        op: u32,
        subs: &[ExprId],
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &c in subs {
            deps |= self.deps(c);
        }
        let list = self.ast.lists.push(subs);
        let op = u8::try_from(op).expect("atomic operation ids are below 256");
        let e = self.simple(
            ExprKind::AtomicExpr,
            ValueCategory::PRValue,
            deps,
            op,
            ty,
            loc,
            list.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// `__builtin_shufflevector(v1, v2, i...)` (clang `ShuffleVectorExpr`;
    /// WP-282): a prvalue of type `ty` whose operands `subs` (the two
    /// vectors, then the indices) are kept as written. `mask` is the
    /// folded indices (`u32::MAX` for a `-1`, a poison lane), `None`
    /// while one is value-dependent; codegen and the Oven read it.
    pub fn shuffle_vector(
        &mut self,
        subs: &[ExprId],
        mask: Option<&[u32]>,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &c in subs {
            deps |= self.deps(c);
        }
        let list = self.ast.lists.push(subs);
        let (known, m) = match mask {
            Some(m) => (1, self.ast.lists.push_raw(m).raw()),
            None => (0, 0),
        };
        let e = self.simple(
            ExprKind::ShuffleVectorExpr,
            ValueCategory::PRValue,
            deps,
            known,
            ty,
            loc,
            list.raw(),
            m,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    // ----- WP-064: expression forms ---------------------------------------

    /// `T()` for a non-class type `T` ([expr.type.conv]/2): a prvalue
    /// value-initialized `ty`; `loc` is the type's location, `rparen`
    /// the closing `)`.
    pub fn scalar_value_init(&mut self, ty: QualType, loc: Loc, rparen: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        let e = self.simple(
            ExprKind::CXXScalarValueInitExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            0,
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// True if `e` is a (possibly parenthesized) `CXXThrowExpr`.
    fn is_throw(&self, mut e: ExprId) -> bool {
        loop {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                k => return k == ExprKind::CXXThrowExpr,
            }
        }
    }

    /// `throw` / `throw e` ([expr.throw]): a prvalue of type `void`
    /// (`ty`); `operand` is the copy-initialized exception object's
    /// source, already converted.
    pub fn throw_expr(&mut self, operand: Option<ExprId>, ty: QualType, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty) | operand.map_or(ExprDeps::NONE, |o| self.deps(o));
        self.simple(
            ExprKind::CXXThrowExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            operand.map_or(0, RawId::raw),
            0,
        )
    }

    /// GNU `x ?: y` ([`ExprKind::BinaryConditionalOperator`]): `common`
    /// is `x`, `opaque` an [`AstBuilder::opaque_value`] of it, `cond` the
    /// contextual conversion of `opaque` to `bool`, `els` the converted
    /// `y`; the true arm is `opaque` (converted to `ty` if needed:
    /// `then`). `vk` is the result's category.
    #[allow(clippy::too_many_arguments, reason = "the four operands plus the header")]
    pub fn binary_conditional(
        &mut self,
        common: ExprId,
        cond: ExprId,
        then: ExprId,
        els: ExprId,
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
    ) -> ExprId {
        let deps = self.deps(common) | self.deps(els) | self.type_deps(ty);
        let list = self.ast.lists.push(&[common, cond, then, els]);
        self.simple(ExprKind::BinaryConditionalOperator, vk, deps, 0, ty, loc, list.raw(), 0)
    }

    /// clang's `PseudoObjectExpr`: `syntactic` is the form as written,
    /// `semantics` the expressions computing it (the `OpaqueValueExpr`s
    /// it binds, each before its uses, then the rest), and
    /// `semantics[result]` the one whose value, type and category are the
    /// whole expression's.
    ///
    /// # Panics
    /// If `result` is out of range.
    pub fn pseudo_object(
        &mut self,
        syntactic: ExprId,
        semantics: &[ExprId],
        result: usize,
    ) -> ExprId {
        let r = *self.ast.exprs.get(semantics[result]);
        let mut deps = self.deps(syntactic);
        for &s in semantics {
            deps |= self.deps(s);
        }
        let mut all = Vec::with_capacity(semantics.len() + 1);
        all.push(syntactic);
        all.extend_from_slice(semantics);
        let list = self.ast.lists.push(&all);
        let idx = u32::try_from(result)
            .expect("invariant: a PseudoObjectExpr has fewer than 2^32 semantic expressions");
        self.push_expr(
            ExprKind::PseudoObjectExpr,
            r.value_category(),
            r.object_kind(),
            ExprFlags::NONE,
            deps,
            0,
            r.ty(),
            self.ast.exprs.get(syntactic).loc(),
            list.raw(),
            idx,
        )
    }

    /// A resolved `co_await` / `co_yield` ([expr.await], [expr.yield]):
    /// `kind` is `CoawaitExpr` or `CoyieldExpr`; the value is
    /// `parts.resume`'s.
    pub fn coawait_expr(
        &mut self,
        kind: ExprKind,
        parts: CoawaitParts,
        implicit: bool,
        suspend: SuspendKind,
        loc: Loc,
    ) -> ExprId {
        let r = *self.ast.exprs.get(parts.resume);
        let deps = r.deps().with(self.deps(parts.operand));
        let p = parts;
        let list =
            self.ast.lists.push(&[p.operand, p.common, p.ready, p.suspend, p.resume, p.opaque]);
        let sk = match suspend {
            SuspendKind::Void => 0u8,
            SuspendKind::Bool => 1,
            SuspendKind::Handle => 2,
        };
        let aux = u8::from(implicit) | (sk << 1);
        self.push_expr(
            kind,
            r.value_category(),
            r.object_kind(),
            ExprFlags::NONE,
            deps,
            aux,
            r.ty(),
            loc,
            list.raw(),
            0,
        )
    }

    /// A `co_await` / `co_yield` in a template pattern: a type-dependent
    /// `DependentCoawaitExpr` (`yield` false) or a `CoyieldExpr` holding
    /// only its operand, rebuilt at instantiation.
    pub fn dependent_coawait(&mut self, operand: ExprId, is_yield: bool, loc: Loc) -> ExprId {
        let deps = self.deps(operand).with(ExprDeps::TYPE_VALUE_INST);
        if is_yield {
            let list = self.ast.lists.push(&[operand]);
            return self.simple(
                ExprKind::CoyieldExpr,
                ValueCategory::PRValue,
                deps,
                0,
                QualType::DEPENDENT,
                loc,
                list.raw(),
                0,
            );
        }
        self.simple(
            ExprKind::DependentCoawaitExpr,
            ValueCategory::PRValue,
            deps,
            0,
            QualType::DEPENDENT,
            loc,
            operand.raw(),
            0,
        )
    }

    /// An lvalue `OpaqueValueExpr` standing for the glvalue `source`
    /// (clang's awaiter binding in `buildCoawaitCalls`).
    pub fn opaque_value_lvalue(&mut self, source: ExprId) -> ExprId {
        let n = *self.ast.exprs.get(source);
        self.push_expr(
            ExprKind::OpaqueValueExpr,
            ValueCategory::LValue,
            n.object_kind(),
            ExprFlags::NONE,
            n.deps(),
            0,
            n.ty(),
            n.loc(),
            source.raw(),
            0,
        )
    }

    /// A GNU statement expression `({ ... })`: `body` is the compound
    /// statement; the value is its last expression statement's (`ty`,
    /// `void` when there is none). `loc` is the `(`, `rparen` the `)`.
    /// `result` is that last expression; `dependent_ctx`: the expression
    /// appears in a dependent context, where it is always value- and
    /// instantiation-dependent (clang's `computeDependence(StmtExpr*,
    /// TemplateDepth)`). A pack is never expanded across it.
    pub fn stmt_expr(
        &mut self,
        body: StmtId,
        ty: QualType,
        result: Option<ExprId>,
        dependent_ctx: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        if let Some(r) = result {
            deps = deps.with(self.deps(r));
        }
        if dependent_ctx {
            deps = deps.with(ExprDeps::VALUE_INST);
        }
        let deps = deps.without(ExprDeps::UNEXPANDED_PACK);
        let e = self.simple(
            ExprKind::StmtExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            body.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// A `LambdaExpr` ([expr.prim.lambda], §5): a prvalue of the closure
    /// type `ty` over the side record `id` (its capture initializers are
    /// the children); `loc` is the `[`, `end` the `}` of the body.
    /// `packs`: the body contains an unexpanded parameter pack of an
    /// enclosing template ([temp.variadic]/5).
    pub fn lambda_expr(
        &mut self,
        id: LambdaId,
        ty: QualType,
        packs: bool,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        if packs {
            deps |= ExprDeps::UNEXPANDED_PACK;
        }
        if let Some(l) = self.ast.exprs.lambdas.get(id).inits {
            let inits: Vec<ExprId> = self.ast.lists.iter::<ExprId>(l).collect();
            for (i, e) in inits.into_iter().enumerate() {
                // A capture pack (`ts...`, `...xs = init`) expands the
                // packs of its initializer ([expr.prim.lambda.capture]/17);
                // an implicit capture of a pack names it only because the
                // body does, which `packs` already accounts for (clang's
                // `LambdaScopeInfo::ContainsUnexpandedParameterPack`).
                let expands = self
                    .ast
                    .exprs
                    .lambdas
                    .get(id)
                    .captures
                    .get(i)
                    .is_some_and(|c| c.ellipsis.is_valid() || c.implicit);
                // Only packs (and errors): the closure type carries the
                // rest, as clang's `computeDependence(LambdaExpr *)`. A
                // never-dependent closure capturing a parameter of a
                // member template is not dependent (spec/sema-lambda.md
                // §9).
                let d = self.deps(e).without(ExprDeps::TYPE_VALUE_INST);
                deps |= if expands { d.without(ExprDeps::UNEXPANDED_PACK) } else { d };
            }
        }
        let e = self.simple(
            ExprKind::LambdaExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            id.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        e
    }

    /// `__builtin_offsetof(T, designator)` ([support.types.layout]/1):
    /// a prvalue of type `std::size_t` (`ty`) whose value is `offset`
    /// bytes. `indices` are the array subscripts written in the
    /// designator (dumped as children, as clang). `invalid_record`: the
    /// designator passes through an invalid class, so the value is not a
    /// constant (clang `ExprConstant` `VisitOffsetOfExpr`).
    pub fn offsetof(
        &mut self,
        ty: QualType,
        indices: &[ExprId],
        offset: u32,
        invalid_record: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &i in indices {
            deps |= self.deps(i);
        }
        let list = self.ast.lists.push(indices);
        let e = self.simple(
            ExprKind::OffsetOfExpr,
            ValueCategory::PRValue,
            deps,
            u8::from(invalid_record),
            ty,
            loc,
            list.raw(),
            offset,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// A value-dependent `__builtin_offsetof(operand, comps...)` in a
    /// template (clang's dependent `OffsetOfExpr`): a `std::size_t`
    /// (`ty`) prvalue, never type-dependent, value-dependent through the
    /// operand type or an index. The designator is kept as written for
    /// instantiation; the index expressions are its children.
    pub fn dependent_offsetof(
        &mut self,
        ty: QualType,
        operand: QualType,
        comps: &[crate::expr::OffsetofComponent],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use crate::expr::OffsetofComponent;
        let mut deps = self.type_deps(ty) | self.type_deps(operand).without(ExprDeps::TYPE);
        let mut indices = Vec::new();
        for c in comps {
            if let OffsetofComponent::Index { index, .. } = *c {
                deps |= self.deps(index).without(ExprDeps::TYPE);
                indices.push(index);
            }
        }
        if deps.intersects(ExprDeps::TYPE_VALUE_INST) {
            deps |= ExprDeps::VALUE_INST;
        }
        let list = self.ast.lists.push(&indices);
        let e = self.simple(
            ExprKind::OffsetOfExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            list.raw(),
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        self.ast.exprs.cold.offsetof_pattern.insert(e, (operand, comps.into())).expect(FRESH_EXPR);
        e
    }

    /// A compound literal `( T ) { ... }` (C99 6.5.2.5, a GNU extension in
    /// C++): `init` is the `InitListExpr`; a prvalue of type `ty`.
    pub fn compound_literal(&mut self, ty: QualType, init: ExprId, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty) | self.deps(init);
        self.simple(
            ExprKind::CompoundLiteralExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            init.raw(),
            0,
        )
    }

    /// A namespace-scope array compound literal `( T[] ) { ... }` (clang
    /// `BuildCompoundLiteralExpr` with `isFileScope`): an lvalue naming an
    /// object of static storage duration, the implicit unnamed internal
    /// variable of `ctx` that it returns second. The variable joins no
    /// member list or lookup table; `init` (the `InitListExpr`) is both
    /// the literal's and the variable's initializer.
    pub fn file_scope_compound_literal(
        &mut self,
        ctx: DeclId,
        ty: QualType,
        init: ExprId,
        loc: Loc,
    ) -> (ExprId, DeclId) {
        let payload = Payload::Var(VarData {
            redecl: self.redecl_link(None),
            init: Some(init),
            spec: VarSpec::new().with_storage(VarStorage::Static),
            eval: VarEval::default(),
            templ: TemplLink::NONE,
        });
        let var = self.ast.decls.push(
            NewDecl {
                kind: DeclKind::Var,
                flags: DeclFlags::IMPLICIT,
                misc: DeclMisc::new(),
                name: DeclName::EMPTY,
                ctx: Some(ctx),
                loc,
                ty: Some(ty),
            },
            payload,
        );
        let deps = self.type_deps(ty) | self.deps(init);
        let e = self.simple(
            ExprKind::CompoundLiteralExpr,
            ValueCategory::LValue,
            deps,
            1,
            ty,
            loc,
            init.raw(),
            var.raw(),
        );
        self.ast.decls.cold.compound_literal.insert(var, e).expect(FRESH_DECL);
        (e, var)
    }

    // ---------------------------------------------------------------------
    // Object construction and temporaries (spec/sema-class.md §5.1)
    // ---------------------------------------------------------------------

    #[allow(clippy::too_many_arguments, reason = "the construct header fields")]
    fn construct_like(
        &mut self,
        kind: ExprKind,
        ty: QualType,
        ctor: DeclId,
        args: &[ExprId],
        info: ConstructInfo,
        flags: ExprFlags,
        loc: Loc,
        end: Option<Loc>,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &a in args {
            deps |= self.deps(a);
        }
        let list = self.ast.lists.push(args);
        let e = self.push_expr(
            kind,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            flags,
            deps,
            info.to_aux(),
            ty,
            loc,
            ctor.raw(),
            list.raw(),
        );
        if let Some(end) = end {
            self.ast.exprs.cold.end_loc.insert(e, end).expect(FRESH_EXPR);
        }
        e
    }

    /// A `CXXConstructExpr` ([class.ctor], [dcl.init]): a prvalue of class
    /// type `ty` calling `ctor` with `args`. `flags` may carry
    /// `HAD_MULTIPLE_CANDIDATES`; `end` is the closing `)`/`}` of a
    /// written initializer.
    #[allow(clippy::too_many_arguments, reason = "the construct header fields")]
    pub fn construct(
        &mut self,
        ty: QualType,
        ctor: DeclId,
        args: &[ExprId],
        info: ConstructInfo,
        flags: ExprFlags,
        loc: Loc,
        end: Option<Loc>,
    ) -> ExprId {
        self.construct_like(ExprKind::CXXConstructExpr, ty, ctor, args, info, flags, loc, end)
    }

    /// A `CXXTemporaryObjectExpr`: `T(args)` / `T{args}` with other than
    /// one argument ([expr.type.conv]/2); `loc` is the type's location,
    /// `end` the closing `)`/`}`.
    #[allow(clippy::too_many_arguments, reason = "the construct header fields")]
    pub fn temporary_object(
        &mut self,
        ty: QualType,
        ctor: DeclId,
        args: &[ExprId],
        info: ConstructInfo,
        flags: ExprFlags,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        self.construct_like(
            ExprKind::CXXTemporaryObjectExpr,
            ty,
            ctor,
            args,
            info,
            flags,
            loc,
            Some(end),
        )
    }

    /// A `CXXInheritedCtorInitExpr`: the base construction in an
    /// inheriting constructor ([class.inhctor.init]); a prvalue of type
    /// `ty`.
    pub fn inherited_ctor_init(
        &mut self,
        ty: QualType,
        ctor: DeclId,
        kind: ConstructionKind,
        inherited_from_vbase: bool,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        let aux = kind as u8 | (u8::from(inherited_from_vbase) << 2);
        self.simple(
            ExprKind::CXXInheritedCtorInitExpr,
            ValueCategory::PRValue,
            deps,
            aux,
            ty,
            loc,
            ctor.raw(),
            0,
        )
    }

    /// A functional-notation cast `T(x)` / `T{x}` ([expr.type.conv]);
    /// `loc` is the type's location, `rparen` the closing `)`/`}`.
    /// Dependence as [`AstBuilder::cast`].
    #[allow(clippy::too_many_arguments, reason = "the cast header fields")]
    pub fn functional_cast(
        &mut self,
        ck: CastKind,
        sub: ExprId,
        ty: QualType,
        vk: ValueCategory,
        path: Option<ListId>,
        loc: Loc,
        rparen: Loc,
        flags: ExprFlags,
    ) -> ExprId {
        let e = self.cast(ExprKind::CXXFunctionalCastExpr, ck, sub, ty, vk, path, loc, flags);
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// Records the type as written `ty` of named or functional cast `e`
    /// ([`crate::expr::ColdExprData::cast_written`]); a second record for
    /// the same cast is ignored.
    pub fn set_cast_written_type(&mut self, e: ExprId, ty: QualType) {
        let _ = self.ast.exprs.cold.cast_written.insert(e, ty);
    }

    /// `dynamic_cast<T>(v)` ([expr.dynamic.cast]); `ck` is `Dynamic`, or
    /// `DerivedToBase` / `NoOp` for the statically resolved forms;
    /// `rparen` is the closing `)`.
    #[allow(clippy::too_many_arguments, reason = "the cast header fields")]
    pub fn dynamic_cast(
        &mut self,
        ck: CastKind,
        sub: ExprId,
        ty: QualType,
        vk: ValueCategory,
        path: Option<ListId>,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let e =
            self.cast(ExprKind::CXXDynamicCastExpr, ck, sub, ty, vk, path, loc, ExprFlags::NONE);
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// Temporary materialization ([conv.rval]): `sub` (a prvalue) as a
    /// glvalue of type `ty`, an lvalue if bound to an lvalue reference,
    /// else an xvalue. `extended_by` is the `LifetimeExtendedTemporary`
    /// of a lifetime-extended temporary ([`AstBuilder::lifetime_extended_temporary`]).
    ///
    /// # Panics
    /// If `extended_by` is given with `FullExpression` storage or omitted
    /// with another storage duration.
    pub fn materialize_temporary(
        &mut self,
        sub: ExprId,
        ty: QualType,
        bound_to_lvalue_ref: bool,
        storage: StorageDuration,
        extended_by: Option<DeclId>,
    ) -> ExprId {
        assert!(
            extended_by.is_some() == (storage != StorageDuration::FullExpression),
            "lifetime extension and storage duration disagree"
        );
        let n = *self.ast.exprs.get(sub);
        let vk = if bound_to_lvalue_ref { ValueCategory::LValue } else { ValueCategory::XValue };
        let aux = storage as u8 | (u8::from(bound_to_lvalue_ref) << 2);
        self.simple(
            ExprKind::MaterializeTemporaryExpr,
            vk,
            n.deps() | self.type_deps(ty),
            aux,
            ty,
            n.loc(),
            sub.raw(),
            extended_by.map_or(0, RawId::raw),
        )
    }

    /// A `std::initializer_list<E>` prvalue of type `ty` over the backing
    /// array `array` (a `MaterializeTemporaryExpr` of type `const E[N]`,
    /// [dcl.init.list]/5).
    pub fn std_initializer_list(&mut self, array: ExprId, ty: QualType) -> ExprId {
        let n = *self.ast.exprs.get(array);
        self.simple(
            ExprKind::CXXStdInitializerListExpr,
            ValueCategory::PRValue,
            n.deps() | self.type_deps(ty),
            0,
            ty,
            n.loc(),
            array.raw(),
            0,
        )
    }

    /// Binds the class prvalue `sub` to a temporary destroyed by `dtor`
    /// at the end of the full-expression ([class.temporary]/4).
    pub fn bind_temporary(&mut self, sub: ExprId, dtor: DeclId) -> ExprId {
        let n = *self.ast.exprs.get(sub);
        self.simple(
            ExprKind::CXXBindTemporaryExpr,
            ValueCategory::PRValue,
            n.deps(),
            0,
            n.ty(),
            n.loc(),
            sub.raw(),
            dtor.raw(),
        )
    }

    /// A full-expression with cleanups ([intro.execution]/5): same type,
    /// category and object kind as `sub`.
    pub fn expr_with_cleanups(&mut self, sub: ExprId, side_effects: bool) -> ExprId {
        let n = *self.ast.exprs.get(sub);
        self.push_expr(
            ExprKind::ExprWithCleanups,
            n.value_category(),
            n.object_kind(),
            ExprFlags::NONE,
            n.deps(),
            u8::from(side_effects),
            n.ty(),
            n.loc(),
            sub.raw(),
            0,
        )
    }

    /// A use of `field`'s default member initializer ([class.base.init]/9)
    /// in context `used_ctx`; `ty` is the field's type without
    /// references and `vk` the category its declared type implies. Always
    /// `rewritten` (as clang 19 dumps it); the initializer is the field's
    /// shared one.
    pub fn default_init(
        &mut self,
        field: DeclId,
        used_ctx: DeclId,
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(ExprKind::CXXDefaultInitExpr, vk, deps, 1, ty, loc, field.raw(), used_ctx.raw())
    }

    /// A use of `param`'s default argument ([dcl.fct.default]) in context
    /// `used_ctx` (the argument is the parameter's shared one); `ty`/`vk`
    /// are the default argument's (sema-overload.md §7).
    pub fn default_arg(
        &mut self,
        param: DeclId,
        used_ctx: DeclId,
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
    ) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(ExprKind::CXXDefaultArgExpr, vk, deps, 0, ty, loc, param.raw(), used_ctx.raw())
    }

    /// `this` ([expr.prim.this]): a prvalue of pointer type `ty`;
    /// `implicit` for a member named without `this->`.
    pub fn this_expr(&mut self, ty: QualType, implicit: bool, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(
            ExprKind::CXXThisExpr,
            ValueCategory::PRValue,
            deps,
            u8::from(implicit),
            ty,
            loc,
            0,
            0,
        )
    }

    /// A call of a non-static member function ([expr.call]/2): `callee`
    /// is the `MemberExpr` (or `.*`/`->*` operator) whose base is the
    /// object argument. Type and category as [`AstBuilder::call`].
    #[allow(clippy::too_many_arguments, reason = "as AstBuilder::call")]
    pub fn member_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
        rparen: Loc,
        flags: ExprFlags,
    ) -> ExprId {
        let mut deps = self.deps(callee) | self.type_deps(ty);
        for &a in args {
            deps |= self.deps(a);
        }
        let list = self.ast.lists.push(args);
        let e = self.push_expr(
            ExprKind::CXXMemberCallExpr,
            vk,
            ObjectKind::Ordinary,
            flags,
            deps,
            0,
            ty,
            loc,
            callee.raw(),
            list.raw(),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// Parenthesized aggregate initialization (P0960, [dcl.init.general]/
    /// 16.6.2.2): a prvalue of type `ty`; `filler` initializes the
    /// remaining array elements; `rparen` is the closing `)`.
    pub fn paren_list_init(
        &mut self,
        ty: QualType,
        inits: &[ExprId],
        filler: Option<ExprId>,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &i in inits.iter().chain(filler.as_ref()) {
            deps |= self.deps(i);
        }
        let list = self.ast.lists.push(inits);
        let e = self.simple(
            ExprKind::CXXParenListInitExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            list.raw(),
            filler.map_or(0, RawId::raw),
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    /// A union's parenthesized aggregate initialization: `init`
    /// copy-initializes the member `field` ([dcl.init.general]/16.6.2.2).
    pub fn paren_list_init_union(
        &mut self,
        ty: QualType,
        init: ExprId,
        field: DeclId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let e = self.paren_list_init(ty, &[init], None, loc, rparen);
        self.ast.exprs.cold.union_field.insert(e, field).expect(FRESH_EXPR);
        e
    }

    /// An `OpaqueValueExpr` standing for `source` (same type, category,
    /// object kind and dependence); `source` is shared, not a child.
    pub fn opaque_value(&mut self, source: ExprId) -> ExprId {
        let n = *self.ast.exprs.get(source);
        self.push_expr(
            ExprKind::OpaqueValueExpr,
            n.value_category(),
            n.object_kind(),
            ExprFlags::NONE,
            n.deps(),
            0,
            n.ty(),
            n.loc(),
            source.raw(),
            0,
        )
    }

    /// An `OpaqueValueExpr` of type `ty` with no source expression: the
    /// exception object a handler's variable is copy-initialized from
    /// ([except.handle]/15), or a defaulted comparison's analyzer operands
    /// (`Xi, Xi`, clang's `DefaultedComparisonAnalyzer`).
    pub fn opaque_value_of(&mut self, ty: QualType, vc: ValueCategory, loc: Loc) -> ExprId {
        self.push_expr(
            ExprKind::OpaqueValueExpr,
            vc,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            ExprDeps::NONE,
            0,
            ty,
            loc,
            0,
            0,
        )
    }

    /// Element-wise array copy ([class.copy.ctor]/14, [expr.prim.lambda.capture]/
    /// 10, structured bindings): `common` is the `OpaqueValueExpr` of the
    /// source array, `elem` the initializer of one element (indexed by an
    /// [`AstBuilder::array_init_index`]); a prvalue of array type `ty`.
    pub fn array_init_loop(&mut self, ty: QualType, common: ExprId, elem: ExprId) -> ExprId {
        let deps = self.type_deps(ty) | self.deps(common) | self.deps(elem);
        let loc = self.ast.exprs.get(common).loc();
        self.simple(
            ExprKind::ArrayInitLoopExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            common.raw(),
            elem.raw(),
        )
    }

    /// A `DesignatedInitUpdateExpr`: `base` initializes the subobject of
    /// type `ty`, then the `updater` list re-initializes the elements
    /// that are not `NoInitExpr` ([dcl.init.aggr]/4, C99 designators).
    pub fn designated_init_update(
        &mut self,
        ty: QualType,
        base: ExprId,
        updater: ExprId,
    ) -> ExprId {
        let deps = self.type_deps(ty) | self.deps(base) | self.deps(updater);
        let loc = self.ast.exprs.get(base).loc();
        self.simple(
            ExprKind::DesignatedInitUpdateExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            base.raw(),
            updater.raw(),
        )
    }

    /// A `NoInitExpr` of type `ty`: an updater element that keeps the
    /// base's value.
    pub fn no_init(&mut self, ty: QualType) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(ExprKind::NoInitExpr, ValueCategory::PRValue, deps, 0, ty, Loc::INVALID, 0, 0)
    }

    /// The current index of the innermost `ArrayInitLoopExpr` (a
    /// `std::size_t` prvalue of type `ty`).
    pub fn array_init_index(&mut self, ty: QualType, loc: Loc) -> ExprId {
        let deps = self.type_deps(ty);
        self.simple(ExprKind::ArrayInitIndexExpr, ValueCategory::PRValue, deps, 0, ty, loc, 0, 0)
    }

    /// `typeid(operand)` ([expr.typeid]): an lvalue of type `ty` (`const
    /// std::type_info`); `potentially_evaluated` for a glvalue of
    /// polymorphic class type. Never type-dependent; value-dependent if
    /// the operand is dependent.
    pub fn typeid_expr(
        &mut self,
        operand: TypeidOperand,
        potentially_evaluated: bool,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let (a, raw, flags) = match operand {
            TypeidOperand::Type(t) => (self.type_deps(t), t.to_raw(), ExprFlags::ARG_IS_TYPE),
            TypeidOperand::Expr(x) => (self.deps(x), x.raw(), ExprFlags::NONE),
        };
        let mut deps = self.type_deps(ty) | a.without(ExprDeps::TYPE);
        if a.contains(ExprDeps::TYPE) {
            deps |= ExprDeps::VALUE_INST;
        }
        let e = self.push_expr(
            ExprKind::CXXTypeidExpr,
            ValueCategory::LValue,
            ObjectKind::Ordinary,
            flags,
            deps,
            u8::from(potentially_evaluated),
            ty,
            loc,
            raw,
            0,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect(FRESH_EXPR);
        e
    }

    // =====================================================================
    // Statements
    // =====================================================================

    fn stmt(&mut self, kind: StmtKind, aux: u16, loc: Loc, a: u32, b: u32) -> StmtId {
        self.ast.stmts.push(StmtNode::new(kind, aux, loc, a, b))
    }

    /// A compound statement `{ ... }` ([stmt.block]).
    pub fn compound(&mut self, stmts: &[StmtId], lbrace: Loc, rbrace: Loc) -> StmtId {
        let list = self.ast.lists.push(stmts);
        self.stmt(StmtKind::CompoundStmt, 0, lbrace, list.raw(), rbrace.to_raw())
    }

    /// A compound statement under FP pragma override `fp` (WP-806).
    pub fn compound_fp(
        &mut self,
        stmts: &[StmtId],
        lbrace: Loc,
        rbrace: Loc,
        fp: crate::fp::FpOverride,
    ) -> StmtId {
        let list = self.ast.lists.push(stmts);
        let aux = self.ast.stmts.intern_fp(fp);
        self.stmt(StmtKind::CompoundStmt, aux, lbrace, list.raw(), rbrace.to_raw())
    }

    /// A declaration statement ([stmt.dcl]).
    pub fn decl_stmt(&mut self, decls: &[DeclId], loc: Loc) -> StmtId {
        let list = self.ast.lists.push(decls);
        self.stmt(StmtKind::DeclStmt, 0, loc, list.raw(), 0)
    }

    /// An expression statement ([stmt.expr]).
    pub fn expr_stmt(&mut self, e: ExprId) -> StmtId {
        let loc = self.ast.exprs.get(e).loc();
        self.stmt(StmtKind::ExprStmt, 0, loc, e.raw(), 0)
    }

    /// A null statement `;` ([stmt.expr]/1).
    pub fn null_stmt(&mut self, loc: Loc) -> StmtId {
        self.stmt(StmtKind::NullStmt, 0, loc, 0, 0)
    }

    /// An `if` statement ([stmt.if]) of form `kind`.
    pub fn if_stmt(&mut self, kind: IfKind, data: IfData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_if(data);
        self.stmt(StmtKind::IfStmt, kind as u16, loc, side, 0)
    }

    /// A `switch` statement ([stmt.switch]).
    pub fn switch_stmt(&mut self, data: SwitchData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_switch(data);
        self.stmt(StmtKind::SwitchStmt, 0, loc, side, 0)
    }

    /// A `case` label ([stmt.label]).
    pub fn case_stmt(&mut self, data: CaseData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_case(data);
        self.stmt(StmtKind::CaseStmt, 0, loc, side, 0)
    }

    /// A `default` label.
    pub fn default_stmt(&mut self, sub: StmtId, loc: Loc) -> StmtId {
        self.stmt(StmtKind::DefaultStmt, 0, loc, sub.raw(), 0)
    }

    /// `break;` ([stmt.break]).
    pub fn break_stmt(&mut self, loc: Loc) -> StmtId {
        self.stmt(StmtKind::BreakStmt, 0, loc, 0, 0)
    }

    /// `continue;` ([stmt.cont]).
    pub fn continue_stmt(&mut self, loc: Loc) -> StmtId {
        self.stmt(StmtKind::ContinueStmt, 0, loc, 0, 0)
    }

    /// A `while` statement ([stmt.while]); `init` and `inc` of `data` are
    /// unused.
    pub fn while_stmt(&mut self, data: LoopData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_loop(data);
        self.stmt(StmtKind::WhileStmt, 0, loc, side, 0)
    }

    /// A `do` statement ([stmt.do]).
    pub fn do_stmt(&mut self, body: StmtId, cond: ExprId, loc: Loc, rparen: Loc) -> StmtId {
        let side = self.ast.stmts.push_loop(LoopData {
            cond: Some(cond),
            body: Some(body),
            ..LoopData::default()
        });
        self.stmt(StmtKind::DoStmt, 0, loc, side, rparen.to_raw())
    }

    /// A `for` statement ([stmt.for]).
    pub fn for_stmt(&mut self, data: LoopData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_loop(data);
        self.stmt(StmtKind::ForStmt, 0, loc, side, 0)
    }

    /// A range-based `for` statement ([stmt.ranged]), with clang's
    /// desugaring in `data`.
    pub fn cxx_for_range_stmt(&mut self, data: RangeForData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_range_for(data);
        self.stmt(StmtKind::CXXForRangeStmt, 0, loc, side, 0)
    }

    /// An expansion statement ([stmt.expand], P1306R5) of `form` whose
    /// synthetic template level is at `depth`; `expanded` once `data`
    /// holds its prelude and expansions.
    pub fn cxx_expansion_stmt(
        &mut self,
        data: ExpansionData,
        form: crate::stmt::ExpansionForm,
        expanded: bool,
        depth: u16,
        loc: Loc,
    ) -> StmtId {
        let side = self.ast.stmts.push_expansion(data);
        let aux = form as u16 | if expanded { crate::stmt::ExpansionForm::EXPANDED } else { 0 };
        self.stmt(StmtKind::CXXExpansionStmt, aux, loc, side, u32::from(depth))
    }

    /// The element `CXXExpansionSelectExpr` initializing the
    /// for-range-declaration of an expansion statement of `form` whose
    /// synthetic template level is at `depth` (a template pattern).
    pub fn cxx_expansion_select(
        &mut self,
        form: crate::stmt::ExpansionForm,
        depth: u16,
        loc: Loc,
    ) -> ExprId {
        self.simple(
            ExprKind::CXXExpansionSelectExpr,
            ValueCategory::PRValue,
            ExprDeps::TYPE_VALUE_INST,
            form as u8,
            QualType::DEPENDENT,
            loc,
            u32::from(depth),
            0,
        )
    }

    /// The body of a coroutine ([dcl.fct.def.coroutine]/5), wrapping the
    /// body as written (`data.body`); `loc` is the body's `{`.
    pub fn coroutine_body_stmt(&mut self, data: CoroutineBodyData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_coro(data);
        self.stmt(StmtKind::CoroutineBodyStmt, 0, loc, side, 0)
    }

    /// `co_return e;` ([stmt.return.coroutine]): `operand` as written,
    /// `promise_call` the call Sema built (`None` in a template pattern);
    /// `implicit` for the `co_return;` at the end of the body.
    pub fn coreturn_stmt(
        &mut self,
        operand: Option<ExprId>,
        promise_call: Option<ExprId>,
        implicit: bool,
        loc: Loc,
    ) -> StmtId {
        let a = operand.map_or(0, RawId::raw);
        let b = promise_call.map_or(0, RawId::raw);
        self.stmt(StmtKind::CoreturnStmt, u16::from(implicit), loc, a, b)
    }

    /// A GNU asm statement (`GCCAsmStmt`, [dcl.asm]); `bits` are the
    /// [`crate::stmt::asm_bits`]. `data.operands` is ignored (the operands
    /// are copied into the operand table).
    pub fn gcc_asm_stmt(
        &mut self,
        data: AsmData,
        operands: &[AsmOperand],
        bits: u16,
        loc: Loc,
    ) -> StmtId {
        let side = self.ast.stmts.push_asm(data, operands);
        self.stmt(StmtKind::GCCAsmStmt, bits, loc, side, 0)
    }

    /// A labeled statement `label: sub` ([stmt.label]); `label` is the
    /// `Label` declaration (`a` = label, `b` = sub-statement).
    pub fn label_stmt(&mut self, label: DeclId, sub: StmtId, loc: Loc) -> StmtId {
        self.stmt(StmtKind::LabelStmt, 0, loc, label.raw(), sub.raw())
    }

    /// An attributed statement `[[attrs]] sub` ([stmt.pre]/1, clang's
    /// `AttributedStmt`): `a` = the `AttrId` list, `b` = the statement.
    /// `loc` is the first attribute-specifier.
    pub fn attributed_stmt(&mut self, attrs: &[AttrId], sub: StmtId, loc: Loc) -> StmtId {
        let list = self.ast.lists.push(attrs);
        self.stmt(StmtKind::AttributedStmt, 0, loc, list.raw(), sub.raw())
    }

    /// `goto label;` ([stmt.goto]); `a` = the `Label` declaration.
    pub fn goto_stmt(&mut self, label: DeclId, loc: Loc) -> StmtId {
        self.stmt(StmtKind::GotoStmt, 0, loc, label.raw(), 0)
    }

    /// GNU `goto *target;` (clang `IndirectGotoStmt`): `a` = the target
    /// (already converted to `const void*`), `b` = the `*` location.
    pub fn indirect_goto_stmt(&mut self, target: ExprId, loc: Loc, star: Loc) -> StmtId {
        self.stmt(StmtKind::IndirectGotoStmt, 0, loc, target.raw(), star.to_raw())
    }

    /// A contract assertion (P2900, spec/contracts.md §4): an
    /// assertion-statement or one function contract specifier. `a` = the
    /// predicate (contextually converted to `bool`), `b` = the
    /// postcondition's `ContractResultName` (0: none), `aux` = the kind
    /// and the evaluation-semantic slot (0). `loc` is the keyword.
    pub fn contract_stmt(
        &mut self,
        kind: crate::stmt::ContractKind,
        cond: ExprId,
        result: Option<DeclId>,
        loc: Loc,
    ) -> StmtId {
        self.stmt(
            StmtKind::ContractAssertStmt,
            kind as u16,
            loc,
            cond.raw(),
            RawId::raw_opt(result),
        )
    }

    /// A try-block ([except.pre]/1): `try compound-statement handler-seq`
    /// (`a` = the compound statement, `b` = the `CXXCatchStmt` list).
    pub fn try_stmt(&mut self, block: StmtId, handlers: &[StmtId], loc: Loc) -> StmtId {
        let list = self.ast.lists.push(handlers);
        self.stmt(StmtKind::CXXTryStmt, 0, loc, block.raw(), list.raw())
    }

    /// A handler `catch ( exception-declaration ) compound-statement`
    /// ([except.handle]); `var` is `None` for `catch (...)`.
    pub fn catch_stmt(&mut self, var: Option<DeclId>, block: StmtId, loc: Loc) -> StmtId {
        self.stmt(StmtKind::CXXCatchStmt, 0, loc, var.map_or(0, RawId::raw), block.raw())
    }

    /// A `return` statement ([stmt.return]) with its NRVO candidate
    /// ([class.copy.elision]/1.1).
    pub fn return_stmt(&mut self, value: Option<ExprId>, nrvo: Option<DeclId>, loc: Loc) -> StmtId {
        self.stmt(
            StmtKind::ReturnStmt,
            0,
            loc,
            value.map_or(0, RawId::raw),
            nrvo.map_or(0, RawId::raw),
        )
    }
}

/// True for the context kinds that own a lookup table (§4.3); functions,
/// blocks and `RequiresExprBody` do not.
fn has_lookup_table(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::TranslationUnit
            | DeclKind::Namespace
            | DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::Enum
    )
}
