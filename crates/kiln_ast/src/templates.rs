//! Template tables (spec/ast.md §8, spec/sema-templates.md §4.1, §11.1).
//!
//! WP-069 lands the parse-time subset: template parameter lists, the
//! template decls' [`TemplateInfo`], template parameter records, the cold
//! default-argument and trailing-requires maps, requires-expression
//! requirements, and the [`AstBuilder`] constructors for template decls and
//! the dependent expression nodes of sema-templates.md §2.4. WP-070a adds
//! the specialization tables (`specs`, `member_specs`, `specializations`,
//! `instantiated_from`, `member_map`; sema-templates.md §4.1) and their
//! constructors ([`AstBuilder::class_spec`], [`AstBuilder::member_of_spec`]).
//! WP-070b adds the partial specialization tables (`partials` with their
//! [`ArgShape`] prefilters, sema-templates.md §4.4) and
//! [`AstBuilder::class_partial_spec`].

use kiln_source::Loc;
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use crate::ast::Ast;
use crate::builder::{AstBuilder, DeclHead};
use crate::decl::{
    DeclFlags, DeclKind, Members, NewDecl, Payload, ScopeData, TemplLink, TemplLinkKind,
};
use crate::expr::{BinaryOp, ExprDeps, ExprFlags, ExprKind, ObjectKind, UnaryOp, ValueCategory};
use crate::ids::{ArgListId, DeclId, ExprId, ListId, NnsId, QualType, RawId, TemplateParamListId};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::names::DeclName;
use crate::slot::ColdMap;
use crate::types::{ArgView, TemplateArg, TypeFlags, TypeKind};

/// A template parameter list ([temp.pre]/1): `template < params >
/// requires-clause?`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateParamList {
    /// The `template` keyword.
    pub template_loc: Loc,
    /// `<`.
    pub l_angle: Loc,
    /// `>`.
    pub r_angle: Loc,
    /// The nesting depth of the parameters ([temp.param]); `template<>`
    /// does not count (spec/sema-templates.md §2.1).
    pub depth: u16,
    /// The parameter decls (`TemplateTypeParm`, `NonTypeTemplateParm`,
    /// `TemplateTemplateParm`), as a `ListId` of `DeclId`s.
    pub params: ListId,
    /// The requires-clause after the head ([temp.pre]/9).
    pub requires: Option<ExprId>,
}

/// What a template decl (`ClassTemplate`, `FunctionTemplate`,
/// `VarTemplate`, `TypeAliasTemplate`, `Concept`) templates. Indexed by
/// the template decl's inline payload and its pattern's
/// [`TemplLink::Pattern`] index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateInfo {
    /// The template decl itself.
    pub decl: DeclId,
    /// Its parameters.
    pub params: TemplateParamListId,
    /// The templated declaration (none for a concept).
    pub pattern: Option<DeclId>,
    /// A concept's constraint-expression ([temp.concept]/1).
    pub constraint: Option<ExprId>,
}

/// A template parameter's position and form ([temp.param]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateParmInfo {
    /// The depth (number of enclosing non-empty template parameter lists).
    pub depth: u16,
    /// The position in its list.
    pub index: u16,
    /// A template parameter pack ([temp.variadic]/1).
    pub pack: bool,
    /// Declared with `typename` rather than `class` (type and template
    /// template parameters).
    pub typename_kw: bool,
    /// A type parameter's immediately-declared constraint ([temp.param]/4),
    /// a `ConceptSpecializationExpr`.
    pub constraint: Option<ExprId>,
    /// A template template parameter's own parameters.
    pub params: Option<TemplateParamListId>,
}

/// A requirement of a requires-expression ([expr.prim.req]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    /// `expr ;` ([expr.prim.req.simple]).
    Simple(ExprId),
    /// `typename T::type ;` ([expr.prim.req.type]).
    Type(QualType),
    /// `{ expr } noexcept? -> type-constraint? ;`
    /// ([expr.prim.req.compound]); `constraint` is the immediately-declared
    /// constraint of the return-type-requirement.
    Compound {
        /// The expression.
        expr: ExprId,
        /// `noexcept` was written.
        noexcept: bool,
        /// The return-type-requirement's constraint.
        constraint: Option<ExprId>,
    },
    /// `requires constraint-expression ;` ([expr.prim.req.nested]).
    Nested(ExprId),
}

/// A requires-expression's body and requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequiresData {
    /// The `RequiresExprBody` decl holding the parameters.
    pub body: DeclId,
    /// The parameters (`ParmVar` decls).
    pub params: ListId,
    first: u32,
    len: u32,
}

/// How a specialization came to be (spec/ast.md §8.2, clang's
/// `TemplateSpecializationKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SpecKind {
    /// Named (`X<int>`) but neither instantiated nor explicitly
    /// specialized yet.
    Undeclared,
    /// Implicitly instantiated ([temp.inst]).
    ImplicitInstantiation,
    /// `template<> struct X<int> {...}` ([temp.expl.spec]).
    ExplicitSpecialization,
    /// `extern template struct X<int>;` ([temp.explicit]).
    ExplicitInstantiationDeclaration,
    /// `template struct X<int>;` ([temp.explicit]).
    ExplicitInstantiationDefinition,
}

/// A class, function or variable template specialization (24 bytes,
/// spec/ast.md §8.2). Indexed by the specialization's
/// [`TemplLink::Specialization`] payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecInfo {
    /// The primary template (its first declaration).
    pub template: DeclId,
    /// The converted arguments, as written (sugared); the canonical list
    /// is `types.canonical_args(args)`.
    pub args: ArgListId,
    /// How the specialization came to be.
    pub kind: SpecKind,
    /// The pattern its definition was instantiated from (the primary
    /// template's pattern definition or a partial specialization); the
    /// template decl until instantiated.
    pub from: DeclId,
    /// The deduced arguments of the partial specialization `from`.
    pub from_args: Option<ArgListId>,
    /// Where the definition was first required ([temp.point]).
    pub point_of_instantiation: Loc,
    /// The arguments names print with: `args` without the trailing
    /// arguments that equal their parameter's default (clang's
    /// `SuppressDefaultTemplateArgs`).
    pub print_args: ArgListId,
}

/// A member of a class template specialization (12 bytes, clang's
/// `MemberSpecializationInfo`). Indexed by the member's
/// [`TemplLink::MemberOfSpecialization`] payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberSpecInfo {
    /// The member of the pattern it was instantiated from.
    pub instantiated_from: DeclId,
    /// How the member came to be.
    pub kind: SpecKind,
    /// Where its definition was first required.
    pub point_of_instantiation: Loc,
}

/// The prefilter shape of one argument of a partial specialization
/// (sema-templates.md §4.4), computed when the partial is declared: a
/// candidate whose argument does not fit is skipped without deduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgShape {
    /// Any argument (a dependent one, or one at or after a pack
    /// expansion).
    Any,
    /// A type whose canonical top level is `kind` with at least the
    /// qualifier bits `quals` (`T *`, `const T`, `T &`).
    Type {
        /// The canonical type class (`None`: any).
        kind: Option<TypeKind>,
        /// cv-qualifier bits the argument must carry.
        quals: u8,
    },
    /// Exactly this canonical argument (a non-dependent one).
    Exact(TemplateArg),
}

impl ArgShape {
    /// The shape of canonical partial-specialization argument `a`.
    fn of(types: &crate::types::TypeContext, a: TemplateArg) -> ArgShape {
        if !types.arg_flags(a).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
            return ArgShape::Exact(a);
        }
        let ArgView::Type(q) = a.view() else { return ArgShape::Any };
        let quals = q.quals_bits();
        let kind = match types.kind(q.ty()) {
            k @ (TypeKind::Pointer
            | TypeKind::LValueRef
            | TypeKind::RValueRef
            | TypeKind::MemberPointer
            | TypeKind::Function) => Some(k),
            TypeKind::CanonTemplateTypeParm if quals != 0 => None,
            _ => return ArgShape::Any,
        };
        ArgShape::Type { kind, quals }
    }

    /// Whether canonical argument `a` fits this shape.
    pub fn admits(self, types: &crate::types::TypeContext, a: TemplateArg) -> bool {
        match self {
            ArgShape::Any => true,
            ArgShape::Exact(x) => x == a,
            ArgShape::Type { kind, quals } => {
                let ArgView::Type(q) = a.view() else { return false };
                q.quals_bits() & quals == quals && kind.is_none_or(|k| types.kind(q.ty()) == k)
            }
        }
    }
}

/// A class template partial specialization ([temp.spec.partial],
/// sema-templates.md §4.1): a `CXXRecord` with a
/// [`TemplLinkKind::PartialSpecialization`] link to this record. It has
/// no template decl of its own (as clang: `getDescribedClassTemplate` is
/// null), so [`Ast::described_template`] is `None` for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialSpecInfo {
    /// The partial specialization's record (its first declaration).
    pub partial: DeclId,
    /// The primary template (its first decl).
    pub template: DeclId,
    /// The partial's template parameter list.
    pub params: TemplateParamListId,
    /// The arguments as written (`X<T *>`).
    pub args: ArgListId,
    /// `types.canonical_args(args)`.
    pub canonical: ArgListId,
    /// Whether any parameter of the partial is a pack (matching such a
    /// partial is WP-072).
    pub has_pack: bool,
    shapes: u32,
    shapes_len: u32,
}

/// Template tables (spec/ast.md §8, M3).
#[derive(Debug)]
pub struct TemplateTables {
    param_lists: Vec<TemplateParamList>,
    infos: Vec<TemplateInfo>,
    parms: Vec<TemplateParmInfo>,
    requirements: Vec<Requirement>,
    requires: Vec<RequiresData>,
    /// Default template arguments, by parameter decl (the cold "template
    /// default args" map, spec/ast.md §8.1).
    pub defaults: ColdMap<DeclId, TemplateArg>,
    /// Trailing requires-clauses of function declarations
    /// ([dcl.decl.general]/2).
    pub trailing_requires: ColdMap<DeclId, ExprId>,
    /// Explicit template arguments of dependent and unresolved names.
    pub expr_targs: ColdMap<ExprId, ArgListId>,
    /// Nested-name-specifiers of dependent and unresolved names.
    pub expr_nns: ColdMap<ExprId, NnsId>,
    /// The first token of a qualified dependent name whose qualifier is
    /// not its own location (clang's `getBeginLoc`).
    pub expr_begin: ColdMap<ExprId, Loc>,
    /// Set by sema right before it creates a template's pattern: the next
    /// declaration the builder adds is neither a member nor a name (its
    /// template decl is both, [`AstBuilder::template_decl`]).
    pub pattern_next: bool,
    /// The number of enclosing template parameter scopes; while non-zero
    /// new declarations get `DEPENDENT_CTX` ([temp.dep]).
    pub dependent_depth: u32,
    specs: Vec<SpecInfo>,
    member_specs: Vec<MemberSpecInfo>,
    /// `(first decl of template, canonical args) -> specialization`
    /// (spec/ast.md §8.3).
    specializations: FxHashMap<(DeclId, ArgListId), DeclId>,
    /// Each template's specializations in creation order (dump order,
    /// ast-dump.md §5), by first decl of the template.
    spec_order: FxHashMap<DeclId, Vec<DeclId>>,
    /// The pattern of instantiated decls without a [`TemplLink`] (fields,
    /// enumerators, parameters, typedefs, locals; spec/ast.md §8.2).
    instantiated_from: FxHashMap<DeclId, DeclId>,
    /// `(specialization, pattern member) -> instantiated member`
    /// (`find_instantiated_decl`, sema-templates.md §5.3).
    member_map: FxHashMap<(DeclId, DeclId), DeclId>,
    /// Partial specializations (§4.4).
    partials: Vec<PartialSpecInfo>,
    /// The prefilter shapes of every partial, by `PartialSpecInfo` range.
    shapes: Vec<ArgShape>,
    /// Each template's partial specializations in declaration order, by
    /// first decl of the template.
    partials_of: FxHashMap<DeclId, Vec<u32>>,
    /// `(template, canonical args) -> partial` (redeclarations).
    partial_map: FxHashMap<(DeclId, ArgListId), u32>,
}

impl Default for TemplateTables {
    fn default() -> Self {
        let dummy_list = TemplateParamList {
            template_loc: Loc::INVALID,
            l_angle: Loc::INVALID,
            r_angle: Loc::INVALID,
            depth: 0,
            params: ListId::EMPTY,
            requires: None,
        };
        let dummy_parm = TemplateParmInfo {
            depth: 0,
            index: 0,
            pack: false,
            typename_kw: false,
            constraint: None,
            params: None,
        };
        Self {
            param_lists: vec![dummy_list],
            infos: Vec::new(),
            parms: vec![dummy_parm],
            requirements: Vec::new(),
            requires: Vec::new(),
            defaults: ColdMap::new("template_defaults"),
            trailing_requires: ColdMap::new("trailing_requires"),
            expr_targs: ColdMap::new("expr_targs"),
            expr_begin: ColdMap::new("expr_begin"),
            expr_nns: ColdMap::new("expr_nns"),
            pattern_next: false,
            dependent_depth: 0,
            specs: Vec::new(),
            member_specs: Vec::new(),
            specializations: FxHashMap::default(),
            spec_order: FxHashMap::default(),
            instantiated_from: FxHashMap::default(),
            member_map: FxHashMap::default(),
            partials: Vec::new(),
            shapes: Vec::new(),
            partials_of: FxHashMap::default(),
            partial_map: FxHashMap::default(),
        }
    }
}

impl TemplateTables {
    /// Template parameter list `l`.
    pub fn param_list(&self, l: TemplateParamListId) -> &TemplateParamList {
        &self.param_lists[l.index() as usize]
    }

    /// Template info `i` (a [`TemplLink::Pattern`] index or a template
    /// decl's payload).
    pub fn info(&self, i: u32) -> Option<&TemplateInfo> {
        self.infos.get(i as usize)
    }

    /// The requirements of requires-expression record `r`.
    pub fn requirements(&self, r: &RequiresData) -> &[Requirement] {
        &self.requirements[r.first as usize..(r.first + r.len) as usize]
    }

    /// Requires-expression record `i`.
    pub fn requires_data(&self, i: u32) -> Option<&RequiresData> {
        self.requires.get(i as usize)
    }

    /// Specialization record `i` (a [`TemplLink::Specialization`] index).
    pub fn spec(&self, i: u32) -> Option<&SpecInfo> {
        self.specs.get(i as usize)
    }

    /// Updates specialization record `i` after instantiation (`kind`,
    /// `from`, `from_args`, point of instantiation). The template and
    /// arguments never change.
    pub fn update_spec(&mut self, i: u32, f: impl FnOnce(&mut SpecInfo)) {
        if let Some(s) = self.specs.get_mut(i as usize) {
            let (t, a) = (s.template, s.args);
            f(s);
            s.template = t;
            s.args = a;
        }
    }

    /// Member-of-specialization record `i`.
    pub fn member_spec(&self, i: u32) -> Option<&MemberSpecInfo> {
        self.member_specs.get(i as usize)
    }

    /// Updates member record `i` (`kind`, point of instantiation).
    pub fn update_member_spec(&mut self, i: u32, f: impl FnOnce(&mut MemberSpecInfo)) {
        if let Some(s) = self.member_specs.get_mut(i as usize) {
            let from = s.instantiated_from;
            f(s);
            s.instantiated_from = from;
        }
    }

    /// The specialization of `template` (its first decl) for the
    /// canonical argument list `canonical`.
    pub fn find_spec(&self, template: DeclId, canonical: ArgListId) -> Option<DeclId> {
        self.specializations.get(&(template, canonical)).copied()
    }

    /// The specializations of `template` (its first decl) in creation
    /// order.
    pub fn specs_of(&self, template: DeclId) -> &[DeclId] {
        self.spec_order.get(&template).map_or(&[], Vec::as_slice)
    }

    /// The number of specializations (of every template).
    pub fn spec_count(&self) -> usize {
        self.specs.len()
    }

    /// The member of specialization `spec` instantiated from the pattern
    /// member `pattern`.
    pub fn find_member(&self, spec: DeclId, pattern: DeclId) -> Option<DeclId> {
        self.member_map.get(&(spec, pattern)).copied()
    }

    /// Records that `inst`, a member of specialization `spec`, was
    /// instantiated from the pattern member `pattern`.
    pub fn record_member(&mut self, spec: DeclId, pattern: DeclId, inst: DeclId) {
        let _ = self.member_map.insert((spec, pattern), inst);
    }

    /// Records the pattern of an instantiated decl that has no
    /// [`TemplLink`] (fields, enumerators, parameters, typedefs, locals).
    pub fn set_instantiated_from(&mut self, inst: DeclId, pattern: DeclId) {
        let _ = self.instantiated_from.insert(inst, pattern);
    }

    /// The pattern recorded by [`TemplateTables::set_instantiated_from`].
    pub fn instantiated_from_map(&self, inst: DeclId) -> Option<DeclId> {
        self.instantiated_from.get(&inst).copied()
    }

    /// Partial specialization record `i`.
    pub fn partial(&self, i: u32) -> Option<&PartialSpecInfo> {
        self.partials.get(i as usize)
    }

    /// The prefilter shapes of partial `p`'s arguments.
    pub fn partial_shapes(&self, p: &PartialSpecInfo) -> &[ArgShape] {
        &self.shapes[p.shapes as usize..(p.shapes + p.shapes_len) as usize]
    }

    /// The partial specializations of `template` (its first decl), in
    /// declaration order, as indices for [`TemplateTables::partial`].
    pub fn partials_of(&self, template: DeclId) -> &[u32] {
        self.partials_of.get(&template).map_or(&[], Vec::as_slice)
    }

    /// The partial specialization of `template` with canonical arguments
    /// `canonical`.
    pub fn find_partial(&self, template: DeclId, canonical: ArgListId) -> Option<u32> {
        self.partial_map.get(&(template, canonical)).copied()
    }

    /// `h` with `DEPENDENT_CTX` added while inside a template.
    pub(crate) fn dependent_head(&self, mut h: DeclHead) -> DeclHead {
        if self.dependent_depth > 0 {
            h.flags = h.flags.with(DeclFlags::DEPENDENT_CTX);
        }
        h
    }
}

impl HeapSize for TemplateTables {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("templ.param_lists", &self.param_lists));
        r.push(MemRow::of_vec("templ.infos", &self.infos));
        r.push(MemRow::of_vec("templ.parms", &self.parms));
        r.push(MemRow::of_vec("templ.requirements", &self.requirements));
        r.push(MemRow::of_vec("templ.requires", &self.requires));
        r.push(self.defaults.mem_row());
        r.push(self.trailing_requires.mem_row());
        r.push(self.expr_targs.mem_row());
        r.push(self.expr_begin.mem_row());
        r.push(self.expr_nns.mem_row());
        r.push(MemRow::of_vec("templ.specs", &self.specs));
        r.push(MemRow::of_vec("templ.member_specs", &self.member_specs));
        r.push(MemRow::of_map("templ.specializations", &self.specializations));
        let order = self.spec_order.values().map(|v| v.capacity() * 4).sum::<usize>() as u64;
        let mut row = MemRow::of_map("templ.spec_order", &self.spec_order);
        row.live_bytes += order;
        row.reserved_bytes += order;
        r.push(row);
        r.push(MemRow::of_map("templ.instantiated_from", &self.instantiated_from));
        r.push(MemRow::of_map("templ.member_map", &self.member_map));
        r.push(MemRow::of_vec("templ.partials", &self.partials));
        r.push(MemRow::of_vec("templ.partial_shapes", &self.shapes));
        let order = self.partials_of.values().map(|v| v.capacity() * 4).sum::<usize>() as u64;
        let mut row = MemRow::of_map("templ.partials_of", &self.partials_of);
        row.live_bytes += order;
        row.reserved_bytes += order;
        r.push(row);
        r.push(MemRow::of_map("templ.partial_map", &self.partial_map));
        r
    }
}

/// True for the template decl kinds that carry a [`TemplateInfo`].
pub const fn is_template_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::ClassTemplate
            | DeclKind::FunctionTemplate
            | DeclKind::VarTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::Concept
    )
}

/// True for the template parameter decl kinds.
pub const fn is_template_parm_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::TemplateTypeParm | DeclKind::NonTypeTemplateParm | DeclKind::TemplateTemplateParm
    )
}

impl Ast {
    /// The template info of template decl `d`.
    pub fn template_info(&self, d: DeclId) -> Option<&TemplateInfo> {
        let n = self.decls.get(d);
        if is_template_kind(n.kind()) { self.templates.info(n.data()) } else { None }
    }

    /// The parameter record of template parameter `d`.
    pub fn template_parm(&self, d: DeclId) -> Option<&TemplateParmInfo> {
        let n = self.decls.get(d);
        if is_template_parm_kind(n.kind()) {
            self.templates.parms.get(n.data() as usize)
        } else {
            None
        }
    }

    /// The [`TemplLink`] of `d` ([`TemplLink::NONE`] for kinds without
    /// one).
    pub fn templ_link(&self, d: DeclId) -> TemplLink {
        let link = match self.decls.get(d).kind().payload() {
            crate::decl::PayloadClass::Function => self.decls.function(d).map(|x| x.templ),
            crate::decl::PayloadClass::Var => self.decls.var(d).map(|x| x.templ),
            crate::decl::PayloadClass::Record => self.decls.record(d).map(|x| x.templ),
            crate::decl::PayloadClass::Enum => self.decls.enumeration(d).map(|x| x.templ),
            crate::decl::PayloadClass::Typedef => self.decls.typedef(d).map(|x| x.templ),
            _ => None,
        };
        link.unwrap_or(TemplLink::NONE)
    }

    /// The specialization record of `d` with its index, when `d` is a
    /// class, function or variable template specialization.
    pub fn spec_info(&self, d: DeclId) -> Option<(u32, &SpecInfo)> {
        match self.templ_link(d).get() {
            Some((TemplLinkKind::Specialization, i)) => Some((i, self.templates.spec(i)?)),
            _ => None,
        }
    }

    /// The template arguments a class template specialization's name
    /// prints with (`None` for other declarations).
    pub fn spec_print_args(&self, d: DeclId) -> Option<ArgListId> {
        if self.decls.get(d).kind() != DeclKind::CXXRecord {
            return None;
        }
        self.spec_info(d).map(|(_, s)| s.print_args)
    }

    /// The member record of `d` with its index, when `d` is a member of a
    /// class template specialization.
    pub fn member_spec_info(&self, d: DeclId) -> Option<(u32, &MemberSpecInfo)> {
        match self.templ_link(d).get() {
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                Some((i, self.templates.member_spec(i)?))
            }
            _ => None,
        }
    }

    /// The pattern `d` was instantiated from (spec/ast.md §8.2): a
    /// specialization's `from`, a member's `instantiated_from`, or the
    /// sparse map.
    pub fn instantiated_from(&self, d: DeclId) -> Option<DeclId> {
        match self.templ_link(d).get() {
            Some((TemplLinkKind::Specialization, i)) => self
                .templates
                .spec(i)
                .map(|s| s.from)
                .filter(|&f| !is_template_kind(self.decls.get(f).kind())),
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                self.templates.member_spec(i).map(|s| s.instantiated_from)
            }
            _ => self.templates.instantiated_from_map(d),
        }
    }

    /// The partial specialization record of `d` with its index, when `d`
    /// is (a declaration of) a class template partial specialization.
    pub fn partial_spec_info(&self, d: DeclId) -> Option<(u32, &PartialSpecInfo)> {
        match self.templ_link(d).get() {
            Some((TemplLinkKind::PartialSpecialization, i)) => {
                Some((i, self.templates.partial(i)?))
            }
            _ => None,
        }
    }

    /// The template decl whose pattern is `d`.
    pub fn described_template(&self, d: DeclId) -> Option<DeclId> {
        let link = self.templ_link(d);
        match link.get() {
            Some((TemplLinkKind::Pattern, i)) => self.templates.info(i).map(|t| t.decl),
            _ => None,
        }
    }

    /// The name an `UnresolvedLookupExpr` looks up.
    ///
    /// # Panics
    /// If `e` is not an `UnresolvedLookupExpr`.
    pub fn unresolved_name(&self, e: ExprId) -> Symbol {
        let n = self.exprs.get(e);
        assert_eq!(n.kind(), ExprKind::UnresolvedLookupExpr, "not an UnresolvedLookupExpr");
        Symbol::from_index(n.a())
    }

    /// The declarations an `UnresolvedLookupExpr` found.
    ///
    /// # Panics
    /// If `e` is not an `UnresolvedLookupExpr`.
    pub fn unresolved_decls(&self, e: ExprId) -> Vec<DeclId> {
        let n = self.exprs.get(e);
        assert_eq!(n.kind(), ExprKind::UnresolvedLookupExpr, "not an UnresolvedLookupExpr");
        self.lists.iter::<DeclId>(ListId::new(n.b())).collect()
    }

    /// The requires-expression record of `RequiresExpr` `e`.
    pub fn requires_of(&self, e: ExprId) -> Option<&RequiresData> {
        let n = self.exprs.get(e);
        if n.kind() == ExprKind::RequiresExpr { self.templates.requires_data(n.b()) } else { None }
    }
}

/// The expression dependence of a template argument.
fn arg_deps(b: &AstBuilder<'_>, a: TemplateArg) -> ExprDeps {
    if let ArgView::Expr { expr, .. } = a.view() {
        return b.deps(expr);
    }
    let f = b.ast.types.arg_flags(a);
    let mut d = ExprDeps::NONE;
    if f.contains(TypeFlags::DEPENDENT) {
        d |= ExprDeps::VALUE_INST;
    }
    if f.contains(TypeFlags::INSTANTIATION_DEPENDENT) {
        d |= ExprDeps::INSTANTIATION;
    }
    if f.contains(TypeFlags::UNEXPANDED_PACK) {
        d |= ExprDeps::UNEXPANDED_PACK;
    }
    d
}

const DEP_ALL: ExprDeps = ExprDeps::TYPE_VALUE_INST;

/// The pack and error bits a nested-name-specifier contributes to the
/// expression it qualifies: `P::value` for a pack `P` contains an
/// unexpanded pack ([temp.variadic]/7).
fn nns_deps(b: &AstBuilder<'_>, nns: Option<NnsId>) -> ExprDeps {
    let Some(n) = nns else { return ExprDeps::NONE };
    let f = b.ast.types.nns_flags(n);
    let mut d = ExprDeps::NONE;
    if f.contains(TypeFlags::UNEXPANDED_PACK) {
        d |= ExprDeps::UNEXPANDED_PACK;
    }
    if f.contains(TypeFlags::CONTAINS_ERROR) {
        d |= ExprDeps::ERROR;
    }
    d
}

impl AstBuilder<'_> {
    // -------------------------------------------------------------------
    // template parameters and decls
    // -------------------------------------------------------------------

    /// A template parameter list.
    pub fn template_param_list(
        &mut self,
        template_loc: Loc,
        l_angle: Loc,
        r_angle: Loc,
        depth: u16,
        params: &[DeclId],
        requires: Option<ExprId>,
    ) -> TemplateParamListId {
        let params = self.ast.lists.push(params);
        let t = &mut self.ast.templates;
        t.param_lists.push(TemplateParamList {
            template_loc,
            l_angle,
            r_angle,
            depth,
            params,
            requires,
        });
        TemplateParamListId::new((t.param_lists.len() - 1) as u32)
    }

    fn parm(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: Option<QualType>,
        info: TemplateParmInfo,
    ) -> DeclId {
        let i = self.ast.templates.parms.len() as u32;
        self.ast.templates.parms.push(info);
        let h = self.ast.templates.dependent_head(h);
        self.ast.decls.push(
            NewDecl {
                kind,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty,
            },
            Payload::None(i),
        )
    }

    /// A type template parameter ([temp.param]/2); its declared type is the
    /// `TemplateTypeParm` type. Not a member of any list or lookup table:
    /// sema scopes it.
    pub fn template_type_parm(
        &mut self,
        h: DeclHead,
        depth: u16,
        index: u16,
        pack: bool,
        typename_kw: bool,
    ) -> DeclId {
        let d = self.ast.decls.next_id();
        let ty = self.ast.types.template_type_parm(d, depth.into(), index.into(), pack);
        let info =
            TemplateParmInfo { depth, index, pack, typename_kw, constraint: None, params: None };
        self.parm(h, DeclKind::TemplateTypeParm, Some(QualType::unqual(ty)), info)
    }

    /// A non-type template parameter of type `ty` ([temp.param]/1).
    pub fn non_type_template_parm(
        &mut self,
        h: DeclHead,
        ty: QualType,
        depth: u16,
        index: u16,
        pack: bool,
    ) -> DeclId {
        let info = TemplateParmInfo {
            depth,
            index,
            pack,
            typename_kw: false,
            constraint: None,
            params: None,
        };
        self.parm(h, DeclKind::NonTypeTemplateParm, Some(ty), info)
    }

    /// A template template parameter with parameters `params`.
    pub fn template_template_parm(
        &mut self,
        h: DeclHead,
        params: TemplateParamListId,
        depth: u16,
        index: u16,
        pack: bool,
        typename_kw: bool,
    ) -> DeclId {
        let info = TemplateParmInfo {
            depth,
            index,
            pack,
            typename_kw,
            constraint: None,
            params: Some(params),
        };
        self.parm(h, DeclKind::TemplateTemplateParm, None, info)
    }

    /// Sets type parameter `d`'s immediately-declared constraint.
    pub fn set_parm_constraint(&mut self, d: DeclId, c: ExprId) {
        let i = self.ast.decls.get(d).data() as usize;
        if let Some(p) = self.ast.templates.parms.get_mut(i) {
            p.constraint = Some(c);
        }
    }

    /// Sets template parameter `d`'s default argument.
    ///
    /// # Errors
    /// If `d` already has one (sema diagnoses "template parameter
    /// redefines default argument" first).
    pub fn set_template_default(
        &mut self,
        d: DeclId,
        a: TemplateArg,
    ) -> Result<(), crate::slot::AlreadySet> {
        self.ast.templates.defaults.insert(d, a)
    }

    /// Adds a template pattern: pushed like [`AstBuilder::add`] but neither
    /// linked into a member list nor entered into lookup.
    pub(crate) fn add_pattern(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: Option<QualType>,
        payload: Payload,
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
        d
    }

    /// The template decl of `kind` for `pattern` (created through
    /// [`TemplateTables::pattern_next`]): same name, contexts, location and
    /// access; it joins the member list and lookup in the pattern's place.
    /// `prev` is the template decl it redeclares.
    pub fn template_decl(
        &mut self,
        kind: DeclKind,
        pattern: DeclId,
        params: TemplateParamListId,
        prev: Option<DeclId>,
    ) -> DeclId {
        let n = *self.ast.decls.get(pattern);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), n.loc());
        h.lexical = self.ast.decls.lexical_ctx(pattern);
        h.misc = n.misc();
        h.flags = n.flags().without(DeclFlags::HAS_LEXICAL_CTX.with(DeclFlags::DEPENDENT_CTX));
        let i = self.ast.templates.infos.len() as u32;
        let d = self.ast.decls.next_id();
        self.ast.templates.infos.push(TemplateInfo {
            decl: d,
            params,
            pattern: Some(pattern),
            constraint: None,
        });
        // The template decl sits in the scope enclosing its own head: it
        // is dependent only inside an outer template.
        let depth = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = depth.saturating_sub(1);
        let d2 = self.add(h, kind, None, Payload::None(i), prev);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        let _ = self.ast.decls.set_templ(pattern, TemplLink::new(TemplLinkKind::Pattern, i));
        if self.ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX) {
            let first = prev.map_or(d, |p| self.ast.first(p));
            self.ast.types.mark_dependent_template(first);
        }
        d
    }

    /// A concept ([temp.concept]): `template<params> concept C = e;`.
    pub fn concept(&mut self, h: DeclHead, params: TemplateParamListId, e: ExprId) -> DeclId {
        let i = self.ast.templates.infos.len() as u32;
        let d = self.ast.decls.next_id();
        self.ast.templates.infos.push(TemplateInfo {
            decl: d,
            params,
            pattern: None,
            constraint: Some(e),
        });
        let depth = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = depth.saturating_sub(1);
        let d2 = self.add(h, DeclKind::Concept, None, Payload::None(i), None);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        d2
    }

    /// The specialization `template<args>` of class template `template`
    /// (sema-templates.md §4.2, clang's `ClassTemplateSpecializationDecl`):
    /// a `CXXRecord` with a [`TemplLink::Specialization`] link
    /// (`SpecKind::Undeclared`), in the template's semantic context but
    /// neither a member nor a name. `canonical` is
    /// `types.canonical_args(args)`; the caller has probed
    /// [`TemplateTables::find_spec`] first.
    pub fn class_spec(
        &mut self,
        template: DeclId,
        args: ArgListId,
        canonical: ArgListId,
        tag: crate::decl::TagKind,
    ) -> DeclId {
        let tmpl = self.ast.first(template);
        let n = *self.ast.decls.get(tmpl);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), n.loc());
        h.misc = n.misc();
        let i = self.ast.templates.specs.len() as u32;
        let d = self.ast.decls.next_id();
        let ty = QualType::unqual(self.ast.types.record_type(d));
        let redecl = crate::decl::Redecl { prev: None, group: self.ast.redecls.new_group(d) };
        let payload = Payload::Record(crate::decl::RecordData {
            redecl,
            members: Members::default(),
            bases: ListId::EMPTY,
            defn: crate::decl::DefinitionBits::empty(),
            templ: TemplLink::new(TemplLinkKind::Specialization, i),
            lambda: None,
            tag,
        });
        // Specializations are never dependent-context decls: they exist
        // only for non-dependent arguments.
        let depth = std::mem::take(&mut self.ast.templates.dependent_depth);
        let d2 = self.add_pattern(h, DeclKind::CXXRecord, Some(ty), payload);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        self.ast.templates.specs.push(SpecInfo {
            template: tmpl,
            args,
            kind: SpecKind::Undeclared,
            from: tmpl,
            from_args: None,
            point_of_instantiation: Loc::INVALID,
            print_args: args,
        });
        let _ = self.ast.templates.specializations.insert((tmpl, canonical), d);
        self.ast.templates.spec_order.entry(tmpl).or_default().push(d);
        d
    }

    /// The class template partial specialization `template<params> class
    /// X<args>` ([temp.spec.partial], sema-templates.md §4.1): a
    /// `CXXRecord` named like the primary `template`, in its semantic
    /// context, a member of `h.lexical` but not a name, with a
    /// [`TemplLinkKind::PartialSpecialization`] link to its
    /// [`PartialSpecInfo`]. The argument shapes of the
    /// matching prefilter (§4.4) are computed here. The caller has probed
    /// [`TemplateTables::find_partial`] first.
    pub fn class_partial_spec(
        &mut self,
        h: DeclHead,
        template: DeclId,
        params: TemplateParamListId,
        args: ArgListId,
        tag: crate::decl::TagKind,
    ) -> DeclId {
        let tmpl = self.ast.first(template);
        let canonical = self.ast.types.canonical_args(args);
        let i = self.ast.templates.partials.len() as u32;
        let d = self.ast.decls.next_id();
        let ty = QualType::unqual(self.ast.types.record_type(d));
        let redecl = crate::decl::Redecl { prev: None, group: self.ast.redecls.new_group(d) };
        let payload = Payload::Record(crate::decl::RecordData {
            redecl,
            members: Members::default(),
            bases: ListId::EMPTY,
            defn: crate::decl::DefinitionBits::empty(),
            templ: TemplLink::new(TemplLinkKind::PartialSpecialization, i),
            lambda: None,
            tag,
        });
        let d2 =
            self.add_partial(h, tmpl, params, args, canonical, DeclKind::CXXRecord, ty, payload);
        debug_assert_eq!(d, d2);
        d
    }

    /// Pushes partial specialization decl `payload` (whose
    /// [`TemplLinkKind::PartialSpecialization`] link is the next
    /// [`PartialSpecInfo`]), appends it to its lexical member list, and
    /// registers it with the matching prefilter shapes (§4.4).
    #[allow(clippy::too_many_arguments)]
    fn add_partial(
        &mut self,
        h: DeclHead,
        tmpl: DeclId,
        params: TemplateParamListId,
        args: ArgListId,
        canonical: ArgListId,
        kind: DeclKind,
        ty: QualType,
        payload: Payload,
    ) -> DeclId {
        let i = self.ast.templates.partials.len() as u32;
        let lexical = h.lexical.unwrap_or(h.ctx);
        let d = self.add_pattern(h, kind, Some(ty), payload);
        if self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, d);
        }
        let first = self.ast.templates.shapes.len() as u32;
        let mut after_pack = false;
        for &a in self.ast.types.args(canonical) {
            let pack = match a.view() {
                ArgView::Type(q) => self.ast.types.kind(q.ty()) == TypeKind::PackExpansion,
                ArgView::TemplateExpansion { .. } | ArgView::Pack(_) => true,
                _ => false,
            } || self.ast.types.arg_flags(a).contains(TypeFlags::UNEXPANDED_PACK);
            after_pack |= pack;
            let shape = if after_pack { ArgShape::Any } else { ArgShape::of(&self.ast.types, a) };
            self.ast.templates.shapes.push(shape);
        }
        let len = self.ast.templates.shapes.len() as u32 - first;
        let plist = self.ast.templates.param_list(params).params;
        let has_pack = self
            .ast
            .lists
            .iter::<DeclId>(plist)
            .any(|p| self.ast.template_parm(p).is_some_and(|i| i.pack));
        self.ast.templates.partials.push(PartialSpecInfo {
            partial: d,
            template: tmpl,
            params,
            args,
            canonical,
            has_pack,
            shapes: first,
            shapes_len: len,
        });
        self.ast.templates.partials_of.entry(tmpl).or_default().push(i);
        let _ = self.ast.templates.partial_map.insert((tmpl, canonical), i);
        d
    }

    /// The specialization `f<args>` of function template `template`
    /// ([temp.fct.spec], sema-templates.md §6.1, clang's
    /// `FunctionTemplateSpecializationInfo`): a function of the pattern's
    /// kind `kind` with function type `ty` and specifiers `spec`, linked
    /// [`TemplLink::Specialization`] (`SpecKind::Undeclared`), in the
    /// template's semantic context but neither a member nor a name.
    /// Parameters are attached afterwards ([`AstBuilder::set_params`]).
    /// `canonical` is `types.canonical_args(args)`; the caller has probed
    /// [`TemplateTables::find_spec`] first.
    pub fn function_spec(
        &mut self,
        template: DeclId,
        args: ArgListId,
        canonical: ArgListId,
        kind: DeclKind,
        ty: QualType,
        spec: crate::decl::FnSpec,
    ) -> DeclId {
        let tmpl = self.ast.first(template);
        let n = *self.ast.decls.get(tmpl);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), n.loc());
        h.misc = n.misc();
        let i = self.ast.templates.specs.len() as u32;
        let d = self.ast.decls.next_id();
        let redecl = crate::decl::Redecl { prev: None, group: self.ast.redecls.new_group(d) };
        let payload = Payload::Function(crate::decl::FunctionData {
            redecl,
            params: ListId::EMPTY,
            body: None,
            spec,
            templ: TemplLink::new(TemplLinkKind::Specialization, i),
        });
        let depth = std::mem::take(&mut self.ast.templates.dependent_depth);
        let d2 = self.add_pattern(h, kind, Some(ty), payload);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        self.ast.templates.specs.push(SpecInfo {
            template: tmpl,
            args,
            kind: SpecKind::Undeclared,
            from: tmpl,
            from_args: None,
            point_of_instantiation: Loc::INVALID,
            print_args: args,
        });
        let _ = self.ast.templates.specializations.insert((tmpl, canonical), d);
        self.ast.templates.spec_order.entry(tmpl).or_default().push(d);
        d
    }

    /// The specialization `v<args>` of variable template `template`
    /// ([temp.spec], sema-templates.md §4.7, clang's
    /// `VarTemplateSpecializationDecl`): a `Var` with a
    /// [`TemplLink::Specialization`] link (`SpecKind::Undeclared`) and
    /// declared type `ty`, in the template's semantic context but neither
    /// a member nor a name. `canonical` is `types.canonical_args(args)`;
    /// the caller has probed [`TemplateTables::find_spec`] first.
    pub fn var_spec(
        &mut self,
        template: DeclId,
        args: ArgListId,
        canonical: ArgListId,
        ty: QualType,
        spec: crate::decl::VarSpec,
    ) -> DeclId {
        let tmpl = self.ast.first(template);
        let n = *self.ast.decls.get(tmpl);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), n.loc());
        h.misc = n.misc();
        let i = self.ast.templates.specs.len() as u32;
        let d = self.ast.decls.next_id();
        let redecl = crate::decl::Redecl { prev: None, group: self.ast.redecls.new_group(d) };
        let payload = Payload::Var(crate::decl::VarData {
            redecl,
            init: None,
            spec,
            eval: crate::decl::VarEval::default(),
            templ: TemplLink::new(TemplLinkKind::Specialization, i),
        });
        let depth = std::mem::take(&mut self.ast.templates.dependent_depth);
        let d2 = self.add_pattern(h, DeclKind::Var, Some(ty), payload);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        self.ast.templates.specs.push(SpecInfo {
            template: tmpl,
            args,
            kind: SpecKind::Undeclared,
            from: tmpl,
            from_args: None,
            point_of_instantiation: Loc::INVALID,
            print_args: args,
        });
        let _ = self.ast.templates.specializations.insert((tmpl, canonical), d);
        self.ast.templates.spec_order.entry(tmpl).or_default().push(d);
        d
    }

    /// The variable template partial specialization `template<params> T
    /// v<args>` ([temp.spec.partial], clang's
    /// `VarTemplatePartialSpecializationDecl`): a `Var` named like the
    /// primary `template`, otherwise as [`AstBuilder::class_partial_spec`].
    pub fn var_partial_spec(
        &mut self,
        h: DeclHead,
        template: DeclId,
        params: TemplateParamListId,
        args: ArgListId,
        ty: QualType,
        spec: crate::decl::VarSpec,
    ) -> DeclId {
        let tmpl = self.ast.first(template);
        let canonical = self.ast.types.canonical_args(args);
        let i = self.ast.templates.partials.len() as u32;
        let d = self.ast.decls.next_id();
        let redecl = crate::decl::Redecl { prev: None, group: self.ast.redecls.new_group(d) };
        let payload = Payload::Var(crate::decl::VarData {
            redecl,
            init: None,
            spec,
            eval: crate::decl::VarEval::default(),
            templ: TemplLink::new(TemplLinkKind::PartialSpecialization, i),
        });
        self.add_partial(h, tmpl, params, args, canonical, DeclKind::Var, ty, payload)
    }

    /// Appends implicit variable template specialization `spec`,
    /// instantiated from `pattern` (the primary's variable or a partial
    /// specialization), to the member list of the pattern's lexical
    /// context: clang lists it there, after the decls present when it is
    /// first referenced (spec/ast-dump.md §5), and locates it at the
    /// pattern.
    pub fn list_implicit_spec(&mut self, spec: DeclId, pattern: DeclId) {
        let p = *self.ast.decls.get(pattern);
        self.ast.decls.set_loc(spec, p.loc());
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let lexical = self.ast.decls.lexical_ctx(pattern).unwrap_or(ctx);
        // The primary's variable sits in its `VarTemplateDecl`.
        let lexical = if self.ast.decls.get(lexical).kind() == DeclKind::VarTemplate {
            self.ast.decls.lexical_ctx(lexical).unwrap_or(ctx)
        } else {
            lexical
        };
        if lexical != ctx {
            let _ = self.ast.decls.set_lexical_ctx(spec, lexical);
        }
        if self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, spec);
        }
    }

    /// Marks specialization `spec` as declared in lexical context
    /// `lexical` (an explicit specialization or instantiation, which
    /// clang lists among the decls of the context it appears in;
    /// spec/ast-dump.md §5). Called once per specialization, when it
    /// first becomes explicit (from `Undeclared` or
    /// `ImplicitInstantiation`); implicit specializations are in no member
    /// list. The specialization is then located at `loc` (its name in the
    /// explicit declaration).
    pub fn declare_spec_in(&mut self, spec: DeclId, lexical: DeclId, loc: Loc) {
        self.ast.decls.set_loc(spec, loc);
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        if lexical != ctx && !self.ast.decls.get(spec).has(DeclFlags::HAS_LEXICAL_CTX) {
            let _ = self.ast.decls.set_lexical_ctx(spec, lexical);
        }
        if self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, spec);
        }
    }

    /// Links `member`, a member of a class template specialization, to the
    /// pattern member `pattern` it was instantiated from
    /// ([`TemplLink::MemberOfSpecialization`]); records it in the member
    /// map of `spec`. Members of kinds without a [`TemplLink`] go to the
    /// sparse `instantiated_from` map instead.
    pub fn member_of_spec(&mut self, spec: DeclId, member: DeclId, pattern: DeclId) {
        use crate::decl::PayloadClass;
        self.ast.templates.record_member(spec, pattern, member);
        let templatable = matches!(
            self.ast.decls.get(member).kind().payload(),
            PayloadClass::Function | PayloadClass::Var | PayloadClass::Record | PayloadClass::Enum
        );
        if templatable {
            let i = self.ast.templates.member_specs.len() as u32;
            if self
                .ast
                .decls
                .set_templ(member, TemplLink::new(TemplLinkKind::MemberOfSpecialization, i))
                .is_ok()
            {
                self.ast.templates.member_specs.push(MemberSpecInfo {
                    instantiated_from: pattern,
                    kind: SpecKind::ImplicitInstantiation,
                    point_of_instantiation: Loc::INVALID,
                });
                return;
            }
        }
        self.ast.templates.set_instantiated_from(member, pattern);
    }

    // -------------------------------------------------------------------
    // dependent expressions (spec/sema-templates.md §2.4)
    // -------------------------------------------------------------------

    #[allow(clippy::unused_self, reason = "one place to change the dependent type's identity")]
    fn dependent_ty(&self) -> QualType {
        QualType::DEPENDENT
    }

    fn side(&mut self, e: ExprId, nns: Option<NnsId>, targs: Option<ArgListId>) {
        if let Some(n) = nns {
            let _ = self.ast.templates.expr_nns.insert(e, n);
        }
        if let Some(t) = targs {
            let _ = self.ast.templates.expr_targs.insert(e, t);
        }
    }

    fn targs_deps(&self, targs: Option<ArgListId>) -> ExprDeps {
        let mut d = ExprDeps::NONE;
        if let Some(l) = targs {
            for &a in self.ast.types.args(l) {
                d |= arg_deps(self, a);
            }
        }
        d
    }

    /// The `RequiresExprBody` of a requires-expression in context `ctx`
    /// ([expr.prim.req]/5): the parameters' declaration context.
    pub fn requires_body(&mut self, ctx: DeclId, loc: Loc) -> DeclId {
        let payload = Payload::Scope(ScopeData { members: Members::default(), aux: 0 });
        // Not a member of `ctx` (clang never lists it among its decls).
        let h = DeclHead::new(ctx, DeclName::EMPTY, loc);
        self.add_unregistered(h, DeclKind::RequiresExprBody, payload, false)
    }

    /// A template argument for expression `e`: dependent expressions get
    /// a stub profile carrying their dependence (WP-070 replaces it with
    /// the canonical profile, spec/types.md §7.2).
    pub fn template_arg_expr(&mut self, e: ExprId) -> TemplateArg {
        let deps = self.deps(e);
        if deps.is_none() {
            return TemplateArg::expr(e, None);
        }
        let mut f = TypeFlags::NONE;
        if deps.intersects(ExprDeps::TYPE.with(ExprDeps::VALUE)) {
            f |= TypeFlags::DEPENDENT_INSTANTIATION;
        }
        if deps.contains(ExprDeps::INSTANTIATION) {
            f |= TypeFlags::INSTANTIATION_DEPENDENT;
        }
        if deps.contains(ExprDeps::UNEXPANDED_PACK) {
            f |= TypeFlags::UNEXPANDED_PACK;
        }
        let p = self.ast.types.profile(&[e.raw()], e, f);
        TemplateArg::expr(e, Some(p))
    }

    /// `T::name` / `X<T>::name<args>` in an expression
    /// ([temp.dep.expr]/3): `DependentScopeDeclRefExpr`, a type-dependent
    /// lvalue.
    pub fn dependent_scope_decl_ref(
        &mut self,
        nns: NnsId,
        name: Symbol,
        targs: Option<ArgListId>,
        begin: Loc,
        loc: Loc,
    ) -> ExprId {
        let ty = self.dependent_ty();
        let deps = DEP_ALL | self.targs_deps(targs) | nns_deps(self, Some(nns));
        let e = self.simple(
            ExprKind::DependentScopeDeclRefExpr,
            ValueCategory::LValue,
            deps,
            0,
            ty,
            loc,
            nns.raw(),
            name.index(),
        );
        self.side(e, None, targs);
        if begin != loc {
            let _ = self.ast.templates.expr_begin.insert(e, begin);
        }
        e
    }

    /// An unresolved function name `f` (for a dependent call or a
    /// function template-id): `UnresolvedLookupExpr` of overloaded
    /// function type naming `decls`; `adl` when argument-dependent lookup
    /// applies at instantiation ([temp.dep.candidate]).
    pub fn unresolved_lookup(
        &mut self,
        name: Symbol,
        decls: &[DeclId],
        adl: bool,
        nns: Option<NnsId>,
        targs: Option<ArgListId>,
        loc: Loc,
    ) -> ExprId {
        self.unresolved_lookup_in(name, decls, adl, nns, targs, false, loc)
    }

    /// [`Self::unresolved_lookup`]; `dependent` for a name that is
    /// type-dependent itself (a member template of the current
    /// instantiation, [temp.dep.expr]/3).
    #[allow(clippy::too_many_arguments, reason = "the lookup fields")]
    pub fn unresolved_lookup_in(
        &mut self,
        name: Symbol,
        decls: &[DeclId],
        adl: bool,
        nns: Option<NnsId>,
        targs: Option<ArgListId>,
        dependent: bool,
        loc: Loc,
    ) -> ExprId {
        let list = self.ast.lists.push(decls);
        let deps = self.targs_deps(targs) | if dependent { DEP_ALL } else { ExprDeps::NONE };
        let deps = if deps.intersects(ExprDeps::VALUE) { deps | DEP_ALL } else { deps };
        let deps = deps | nns_deps(self, nns);
        // clang's `OverloadExpr`: a type-dependent name (dependent explicit
        // arguments) has the dependent type.
        let ty =
            if deps.contains(ExprDeps::TYPE) { self.dependent_ty() } else { QualType::OVERLOAD };
        let e = self.simple(
            ExprKind::UnresolvedLookupExpr,
            ValueCategory::LValue,
            deps,
            u8::from(adl),
            ty,
            loc,
            name.index(),
            list.raw(),
        );
        self.side(e, nns, targs);
        e
    }

    /// `base.name` / `base->name` with a type-dependent object
    /// ([temp.dep.expr]/5): `CXXDependentScopeMemberExpr`.
    #[allow(clippy::too_many_arguments, reason = "the member-access fields")]
    pub fn dependent_member(
        &mut self,
        base: Option<ExprId>,
        arrow: bool,
        name: Symbol,
        template_kw: bool,
        nns: Option<NnsId>,
        targs: Option<ArgListId>,
        loc: Loc,
    ) -> ExprId {
        let ty = self.dependent_ty();
        let deps = DEP_ALL
            | base.map_or(ExprDeps::NONE, |b| self.deps(b))
            | self.targs_deps(targs)
            | nns_deps(self, nns);
        let aux = u8::from(arrow) | (u8::from(template_kw) << 1);
        let e = self.simple(
            ExprKind::CXXDependentScopeMemberExpr,
            ValueCategory::LValue,
            deps,
            aux,
            ty,
            loc,
            ExprId::raw_opt(base),
            name.index(),
        );
        self.side(e, nns, targs);
        e
    }

    /// `T(args)` / `T{args}` with a dependent `T` ([expr.type.conv]):
    /// `CXXUnresolvedConstructExpr`; `list` for the braced form.
    pub fn unresolved_construct(
        &mut self,
        ty: QualType,
        args: &[ExprId],
        list: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &a in args {
            deps |= self.deps(a);
        }
        let vk = self.ref_category(ty);
        let l = self.ast.lists.push(args);
        let e = self.simple(
            ExprKind::CXXUnresolvedConstructExpr,
            vk,
            deps,
            u8::from(list),
            ty,
            loc,
            l.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rparen);
        e
    }

    fn ref_category(&self, ty: QualType) -> ValueCategory {
        use crate::types::TypeView;
        match self.ast.types.view(ty.ty()) {
            TypeView::LValueRef(_) => ValueCategory::LValue,
            TypeView::RValueRef(_) => ValueCategory::XValue,
            _ => ValueCategory::PRValue,
        }
    }

    /// A built-in unary operator on a type-dependent operand
    /// ([temp.dep.expr]/1, resolved at instantiation): `<dependent type>`,
    /// an lvalue for `*` and prefix `++`/`--` (clang's `CreateOverloadedUnaryOp`
    /// with no visible operator functions), else a prvalue.
    pub fn dependent_unary(&mut self, op: UnaryOp, operand: ExprId, loc: Loc) -> ExprId {
        let vk = if matches!(op, UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::Deref) {
            ValueCategory::LValue
        } else {
            ValueCategory::PRValue
        };
        let deps = self.deps(operand) | DEP_ALL;
        self.push_expr(
            ExprKind::UnaryOperator,
            vk,
            ObjectKind::Ordinary,
            ExprFlags::CANNOT_OVERFLOW,
            deps,
            op as u8,
            self.dependent_ty(),
            loc,
            operand.raw(),
            0,
        )
    }

    /// A built-in binary operator with a type-dependent operand
    /// ([temp.dep.expr]/1): a `<dependent type>` prvalue, or for a
    /// compound assignment an lvalue whose computation types are
    /// dependent too.
    pub fn dependent_binary(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId, loc: Loc) -> ExprId {
        let ty = self.dependent_ty();
        if op.is_compound_assignment() {
            return self.compound_assign(op, lhs, rhs, ty, ty, ty, loc);
        }
        let deps = self.deps(lhs) | self.deps(rhs) | DEP_ALL;
        self.simple(
            ExprKind::BinaryOperator,
            ValueCategory::PRValue,
            deps,
            op as u8,
            ty,
            loc,
            lhs.raw(),
            rhs.raw(),
        )
    }

    /// `c ? t : e` with a type-dependent arm ([temp.dep.expr]/1): a
    /// `<dependent type>` prvalue.
    pub fn dependent_conditional(
        &mut self,
        cond: ExprId,
        then: ExprId,
        els: ExprId,
        loc: Loc,
    ) -> ExprId {
        let deps = self.deps(cond) | self.deps(then) | self.deps(els) | DEP_ALL;
        let side = self.ast.exprs.push_cond_arms(then, els);
        let ty = self.dependent_ty();
        self.simple(
            ExprKind::ConditionalOperator,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            loc,
            cond.raw(),
            side,
        )
    }

    /// `( exprs )` kept unresolved as a dependent initializer
    /// ([dcl.init.general]/16 is applied at instantiation): `ParenListExpr`.
    pub fn paren_list(&mut self, exprs: &[ExprId], l: Loc, r: Loc) -> ExprId {
        let mut deps = ExprDeps::NONE;
        for &a in exprs {
            deps |= self.deps(a);
        }
        let ty = self.dependent_ty();
        let list = self.ast.lists.push(exprs);
        let e = self.simple(
            ExprKind::ParenListExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            l,
            list.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, r);
        e
    }

    /// `sizeof...(pack)` ([expr.sizeof]/5): `SizeOfPackExpr`, a
    /// value-dependent prvalue of type `size_t`.
    pub fn size_of_pack(
        &mut self,
        pack: DeclId,
        size_t: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let e = self.simple(
            ExprKind::SizeOfPackExpr,
            ValueCategory::PRValue,
            ExprDeps::VALUE_INST,
            0,
            size_t,
            loc,
            pack.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rparen);
        e
    }

    /// `pattern...` ([temp.variadic]/5): `PackExpansionExpr` of dependent
    /// type with the pattern's category; the expansion has no unexpanded
    /// packs of its own.
    pub fn pack_expansion_expr(&mut self, pattern: ExprId, ellipsis: Loc) -> ExprId {
        let deps = (self.deps(pattern) | DEP_ALL).without(ExprDeps::UNEXPANDED_PACK);
        let vk = self.ast.exprs.get(pattern).value_category();
        let ty = self.dependent_ty();
        self.simple(ExprKind::PackExpansionExpr, vk, deps, 0, ty, ellipsis, pattern.raw(), 0)
    }

    /// `pack...[index]` (C++26 [expr.prim.pack.index]): `PackIndexingExpr`
    /// of dependent type, located at the pack's name and ending at `]`.
    /// The pack is expanded by the indexing; packs in `index` stay.
    pub fn pack_indexing_expr(&mut self, pattern: ExprId, index: ExprId, rsquare: Loc) -> ExprId {
        let deps =
            ((self.deps(pattern) | DEP_ALL).without(ExprDeps::UNEXPANDED_PACK)) | self.deps(index);
        let n = *self.ast.exprs.get(pattern);
        let ty = self.dependent_ty();
        let e = self.simple(
            ExprKind::PackIndexingExpr,
            ValueCategory::LValue,
            deps,
            0,
            ty,
            n.loc(),
            pattern.raw(),
            index.raw(),
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rsquare);
        e
    }

    /// A fold-expression ([expr.prim.fold]): `CXXFoldExpr`, a dependent
    /// prvalue. `lhs`/`rhs` are the operands as written (`( ... op e )` has
    /// no `lhs`, `( e op ... )` no `rhs`).
    pub fn fold_expr(
        &mut self,
        lhs: Option<ExprId>,
        op: BinaryOp,
        rhs: Option<ExprId>,
        lparen: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = DEP_ALL;
        for e in [lhs, rhs].into_iter().flatten() {
            deps |= self.deps(e);
        }
        let deps = deps.without(ExprDeps::UNEXPANDED_PACK);
        let ty = self.dependent_ty();
        let e = self.simple(
            ExprKind::CXXFoldExpr,
            ValueCategory::PRValue,
            deps,
            op as u8,
            ty,
            lparen,
            ExprId::raw_opt(lhs),
            ExprId::raw_opt(rhs),
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rparen);
        e
    }

    /// A requires-expression ([expr.prim.req]): a `bool` prvalue,
    /// value-dependent when a requirement is.
    pub fn requires_expr(
        &mut self,
        body: DeclId,
        params: &[DeclId],
        reqs: &[Requirement],
        bool_ty: QualType,
        loc: Loc,
        rbrace: Loc,
    ) -> ExprId {
        let mut dep = false;
        for r in reqs {
            dep |= match *r {
                Requirement::Simple(e) | Requirement::Nested(e) => !self.deps(e).is_none(),
                Requirement::Compound { expr, .. } => !self.deps(expr).is_none(),
                Requirement::Type(t) => !self.type_deps(t).is_none(),
            };
        }
        let t = &mut self.ast.templates;
        let first = t.requirements.len() as u32;
        t.requirements.extend_from_slice(reqs);
        let params = self.ast.lists.push(params);
        let t = &mut self.ast.templates;
        let i = t.requires.len() as u32;
        t.requires.push(RequiresData { body, params, first, len: reqs.len() as u32 });
        let deps = if dep { ExprDeps::VALUE_INST } else { ExprDeps::NONE };
        let e = self.simple(
            ExprKind::RequiresExpr,
            ValueCategory::PRValue,
            deps,
            0,
            bool_ty,
            loc,
            body.raw(),
            i,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rbrace);
        e
    }

    /// A substituted non-type template parameter reference
    /// ([temp.inst]; clang's `SubstNonTypeTemplateParmExpr`): `param` is
    /// the `NonTypeTemplateParmDecl` the pattern named and `replacement`
    /// the converted argument expression. It has the replacement's
    /// category and type and none of the pattern's dependence.
    pub fn subst_nttp_expr(&mut self, param: DeclId, replacement: ExprId, loc: Loc) -> ExprId {
        let n = *self.ast.exprs.get(replacement);
        self.push_expr(
            ExprKind::SubstNonTypeTemplateParmExpr,
            n.value_category(),
            n.object_kind(),
            ExprFlags::NONE,
            n.deps(),
            0,
            n.ty(),
            loc,
            param.raw(),
            replacement.raw(),
        )
    }

    /// `C<args>` naming concept `c` ([temp.names]/8): a
    /// `ConceptSpecializationExpr`, a `bool` prvalue, value-dependent when
    /// an argument is.
    pub fn concept_spec_expr(
        &mut self,
        c: DeclId,
        args: ArgListId,
        bool_ty: QualType,
        loc: Loc,
    ) -> ExprId {
        let d = self.targs_deps(Some(args));
        let deps = if d.is_none() { ExprDeps::NONE } else { ExprDeps::VALUE_INST.with(d) };
        let deps = deps.without(ExprDeps::TYPE);
        self.push_expr(
            ExprKind::ConceptSpecializationExpr,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            deps,
            0,
            bool_ty,
            loc,
            c.raw(),
            args.raw(),
        )
    }
}
