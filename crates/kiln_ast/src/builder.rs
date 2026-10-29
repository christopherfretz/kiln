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

use crate::ast::Ast;
use crate::decl::{
    Access, CtorInit, DeclFlags, DeclKind, DeclMisc, DefinitionBits, EnumData, EnumeratorData,
    FieldData, FnSpec, FriendKind, FriendTarget, FunctionData, LetData, LinkageLang, Members,
    NamespaceData, NewDecl, ParamData, Payload, PayloadClass, RecordData, Redecl, ScopeData,
    StaticAssertData, TagKind, TemplLink, TypedefData, VarData, VarEval, VarSpec,
};
use crate::expr::{
    BinaryOp, CastKind, CharKind, CompoundAssignExtra, ConstructInfo, ConstructionKind,
    DeclRefExtra, ExprDeps, ExprFlags, ExprKind, ExprNode, InitListExtra, NonOdrUse, ObjectKind,
    Radix, StorageDuration, StringKind, TraitKind, TypeidOperand, UnaryOp, ValueCategory,
};
use crate::ids::{
    AttrId, ConstValueId, DeclId, ExprId, ListId, QualType, RawId, StmtId, StringLitId, TypeId,
};
use crate::lookup::LookupFlags;
use crate::names::DeclName;
use crate::slot::AlreadySet;
use crate::stmt::{CaseData, IfData, IfKind, LoopData, StmtKind, StmtNode, SwitchData};
use crate::types::TypeFlags;

/// Builtin `TypeId` of the `Dependent` placeholder (spec/types.md §5).
pub const BUILTIN_DEPENDENT: u32 = 29;

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
    ast: &'a mut Ast,
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
    fn add(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: Option<QualType>,
        payload: Payload,
        prev: Option<DeclId>,
    ) -> DeclId {
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
            self.ast
                .decls
                .set_lexical_ctx(d, lex)
                .expect("fresh declaration has no lexical context");
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
    fn add_unregistered(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        payload: Payload,
        member: bool,
    ) -> DeclId {
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
            self.ast
                .decls
                .set_lexical_ctx(d, lex)
                .expect("fresh declaration has no lexical context");
        }
        let lexical = h.lexical.unwrap_or(h.ctx);
        if member && self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, d);
        }
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

    fn is_inline_namespace(&self, ns: DeclId) -> bool {
        self.ast.decls.namespace(self.ast.first(ns)).is_some_and(NamespaceData::is_inline)
    }

    /// The redeclaration link of a new declaration: joins `prev`'s group
    /// (after its latest declaration) or starts a group at the id the next
    /// push returns.
    fn redecl_link(&mut self, prev: Option<DeclId>) -> Redecl {
        let d = self.ast.decls.next_id();
        match prev {
            Some(p) => {
                let group = self.ast.decls.redecl(p).expect("prev is not redeclarable").group;
                let latest = self.ast.redecls.get(group).latest;
                self.ast.redecls.push_redecl(group, d);
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
        let replaces = self.latest_of(prev);
        let redecl = self.redecl_link(prev);
        let ty = if let Some(p) = prev {
            self.ast.decls.get(self.ast.first(p)).ty()
        } else {
            let d = self.ast.decls.next_id();
            Some(QualType::unqual(self.ast.types.record_type(d)))
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
            Some(QualType::unqual(self.ast.types.enum_type(d)))
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
        let payload =
            Payload::Param(ParamData { default_arg: None, original_ty: None, index, flags });
        self.add(DeclHead::new(func, name, loc), DeclKind::ParmVar, Some(ty), payload, None)
    }

    /// Sets the parameter list of `func`.
    ///
    /// # Errors
    /// If a non-empty list was already set.
    pub fn set_params(&mut self, func: DeclId, params: &[DeclId]) -> Result<(), AlreadySet> {
        let list = self.ast.lists.push(params);
        self.ast.decls.set_params(func, list)
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

    // =====================================================================
    // Expressions
    // =====================================================================

    /// The dependence implied by type `ty` (§5.1).
    pub fn type_deps(&self, ty: QualType) -> ExprDeps {
        deps_of_type_flags(self.ast.types.flags(ty.ty()))
    }

    fn deps(&self, e: ExprId) -> ExprDeps {
        self.ast.exprs.get(e).deps()
    }

    fn vk(&self, e: ExprId) -> ValueCategory {
        self.ast.exprs.get(e).value_category()
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors the 10 header fields of ExprNode")]
    fn push_expr(
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
    fn simple(
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

    /// The value category of an id-expression naming `d`
    /// ([expr.prim.id.unqual]/3): lvalue for variables, functions, data
    /// members and template parameter objects; prvalue for enumerators and
    /// non-type template parameters ([temp.param]/8 — class-type ones name
    /// a `TemplateParamObject` once substituted).
    fn decl_ref_category(&self, d: DeclId) -> ValueCategory {
        match self.ast.decls.get(d).kind() {
            DeclKind::EnumConstant | DeclKind::NonTypeTemplateParm => ValueCategory::PRValue,
            // A builtin function's name has the placeholder type
            // `<builtin fn type>` and is a prvalue (as clang).
            DeclKind::Function if self.ast.decls.builtin_id(d).is_some() => ValueCategory::PRValue,
            _ => ValueCategory::LValue,
        }
    }

    /// Dependence an id-expression gets from the declaration it names
    /// beyond its type ([temp.dep.constexpr]/2): a non-type template
    /// parameter is value-dependent; so is a potentially-constant variable
    /// with a value-dependent initializer. A declaration in a dependent
    /// context makes the reference instantiation-dependent.
    fn decl_deps(&self, d: DeclId) -> ExprDeps {
        let n = self.ast.decls.get(d);
        let mut deps = ExprDeps::NONE;
        if n.has(DeclFlags::DEPENDENT_CTX) {
            deps |= ExprDeps::INSTANTIATION;
        }
        match n.kind() {
            DeclKind::NonTypeTemplateParm => deps |= ExprDeps::VALUE_INST,
            DeclKind::Var => {
                if let Some(v) = self.ast.decls.var(d) {
                    let potentially_constant =
                        v.spec.is_constexpr() || n.ty().is_some_and(|t| t.quals_bits() & 1 != 0);
                    if potentially_constant
                        && v.init.is_some_and(|i| self.deps(i).contains(ExprDeps::VALUE))
                    {
                        deps |= ExprDeps::VALUE_INST;
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
        let deps = self.type_deps(ty) | self.decl_deps(d);
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
    /// are lvalues; `.` on a field takes the base's category, a prvalue
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
        let deps = self.deps(base) | self.type_deps(ty) | self.decl_deps(m);
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
            self.ast.exprs.cold.member_extra.insert(e, x).expect("fresh expression");
        }
        e
    }

    /// A built-in unary operator ([expr.unary.op], [expr.post.incr],
    /// [expr.pre.incr]): `*` and prefix `++`/`--` are lvalues, `__real` /
    /// `__imag` / `__extension__` keep the operand's category, the rest are
    /// prvalues.
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
            UnaryOp::Real | UnaryOp::Imag | UnaryOp::Extension => self.vk(operand),
            _ => ValueCategory::PRValue,
        };
        let deps = self.deps(operand) | self.type_deps(ty);
        self.push_expr(
            ExprKind::UnaryOperator,
            vk,
            ObjectKind::Ordinary,
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
        let x = self.vk(base) == ValueCategory::XValue || self.vk(idx) == ValueCategory::XValue;
        let vk = if x { ValueCategory::XValue } else { ValueCategory::LValue };
        let deps = self.deps(base) | self.deps(idx) | self.type_deps(ty);
        self.simple(ExprKind::ArraySubscriptExpr, vk, deps, 0, ty, loc, base.raw(), idx.raw())
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
        e
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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

    /// A GNU statement expression `({ ... })`: `body` is the compound
    /// statement; the value is its last expression statement's (`ty`,
    /// `void` when there is none). `loc` is the `(`, `rparen` the `)`.
    pub fn stmt_expr(&mut self, body: StmtId, ty: QualType, loc: Loc, rparen: Loc) -> ExprId {
        let deps = self.type_deps(ty);
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
        e
    }

    /// `__builtin_offsetof(T, designator)` ([support.types.layout]/1):
    /// a prvalue of type `std::size_t` (`ty`) whose value is `offset`
    /// bytes. `indices` are the array subscripts written in the
    /// designator (dumped as children, as clang).
    pub fn offsetof(
        &mut self,
        ty: QualType,
        indices: &[ExprId],
        offset: u32,
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
            0,
            ty,
            loc,
            list.raw(),
            offset,
        );
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
            self.ast.exprs.cold.end_loc.insert(e, end).expect("fresh expression");
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
        e
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
        self.ast.exprs.cold.end_loc.insert(e, rparen).expect("fresh expression");
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
    pub fn do_stmt(&mut self, body: StmtId, cond: ExprId, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_loop(LoopData {
            cond: Some(cond),
            body: Some(body),
            ..LoopData::default()
        });
        self.stmt(StmtKind::DoStmt, 0, loc, side, 0)
    }

    /// A `for` statement ([stmt.for]).
    pub fn for_stmt(&mut self, data: LoopData, loc: Loc) -> StmtId {
        let side = self.ast.stmts.push_loop(data);
        self.stmt(StmtKind::ForStmt, 0, loc, side, 0)
    }

    /// A labeled statement `label: sub` ([stmt.label]); `label` is the
    /// `Label` declaration (`a` = label, `b` = sub-statement).
    pub fn label_stmt(&mut self, label: DeclId, sub: StmtId, loc: Loc) -> StmtId {
        self.stmt(StmtKind::LabelStmt, 0, loc, label.raw(), sub.raw())
    }

    /// `goto label;` ([stmt.goto]); `a` = the `Label` declaration.
    pub fn goto_stmt(&mut self, label: DeclId, loc: Loc) -> StmtId {
        self.stmt(StmtKind::GotoStmt, 0, loc, label.raw(), 0)
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
