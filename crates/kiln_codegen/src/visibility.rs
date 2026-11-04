//! Symbol visibility and `dso_local` (spec/codegen.md "Symbol
//! visibility"): clang's `CodeGenModule::setGlobalVisibility`,
//! `getLVForDecl` and `shouldAssumeDSOLocal`.

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::consts::{ConstValue, PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TemplateName, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_llvm::{Linkage, Module, Visibility};

use crate::RelocModel;
use crate::module::Cg;

/// clang's `LinkageInfo` visibility part: a visibility and whether it
/// came from an attribute (a declaration's own, its class's, its
/// namespace's or a `#pragma GCC visibility`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lv {
    pub vis: Visibility,
    pub explicit: bool,
}

/// Hidden < protected < default (clang's `Visibility` order).
const fn rank(v: Visibility) -> u8 {
    match v {
        Visibility::Hidden => 0,
        Visibility::Protected => 1,
        Visibility::Default => 2,
    }
}

impl Lv {
    const DEFAULT: Lv = Lv { vis: Visibility::Default, explicit: false };

    /// clang `LinkageInfo::mergeVisibility`: never raises visibility;
    /// an equal one only adds explicitness.
    fn merge_vis(&mut self, vis: Visibility, explicit: bool) {
        if rank(self.vis) < rank(vis) || (self.vis == vis && !explicit) {
            return;
        }
        *self = Lv { vis, explicit };
    }

    fn merge(&mut self, o: Lv) {
        self.merge_vis(o.vis, o.explicit);
    }
}

/// clang's `LVComputationKind`: type visibility (vtables, RTTI) or value
/// visibility, and `IgnoreExplicitVisibility` (a member with its own
/// attribute computing its class's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Comp {
    ty: bool,
    ignore_explicit: bool,
}

impl Comp {
    const VALUE: Comp = Comp { ty: false, ignore_explicit: false };
    const TYPE: Comp = Comp { ty: true, ignore_explicit: false };

    const fn key(self) -> u8 {
        self.ty as u8 | (self.ignore_explicit as u8) << 1
    }
}

const fn llvm_vis(a: u32) -> Visibility {
    match a {
        1 => Visibility::Hidden,
        2 => Visibility::Protected,
        _ => Visibility::Default,
    }
}

const fn is_record_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

const fn is_var_kind(k: DeclKind) -> bool {
    matches!(k, DeclKind::Var | DeclKind::VarTemplateSpecialization)
}

const fn is_explicit_inst_or_spec(k: SpecKind) -> bool {
    matches!(
        k,
        SpecKind::ExplicitSpecialization
            | SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition
    )
}

/// The inputs of clang's `shouldAssumeDSOLocal` for one global value.
#[allow(clippy::struct_excessive_bools)]
struct GvFacts<'n> {
    name: &'n str,
    linkage: Linkage,
    visibility: Visibility,
    /// A declaration for the linker (`available_externally` counts).
    decl: bool,
    function: bool,
    tls: bool,
    comdat: bool,
}

/// The `dso_local` decision of clang 19's `shouldAssumeDSOLocal` (ELF):
/// `None` keeps what emission chose (a definition under `-fpie`).
fn assume_dso_local(
    g: &GvFacts<'_>,
    reloc: RelocModel,
    semantic_interposition: Option<bool>,
    direct_access: bool,
) -> Option<bool> {
    if g.linkage.is_local()
        || (g.visibility != Visibility::Default && g.linkage != Linkage::ExternWeak)
    {
        // Implied by the linkage or visibility; the printer omits it.
        return None;
    }
    // An intrinsic, or the `extern_weak` `_ZTH` declaration (clang
    // creates it without `setGVProperties`; kiln has no other
    // `extern_weak` symbol).
    if g.name.starts_with("llvm.") || g.linkage == Linkage::ExternWeak {
        return Some(false);
    }
    match reloc {
        // Not an executable: only a function definition that can use a
        // local alias, under `-fno-semantic-interposition`
        // (`GlobalValue::canBenefitFromLocalAlias`).
        RelocModel::Pic(_) => Some(
            semantic_interposition == Some(false)
                && g.function
                && !g.decl
                && g.linkage == Linkage::External
                && !g.comdat,
        ),
        // A definition cannot be preempted from an executable. Under
        // `-fpie` clang decides when it creates the global, which may be
        // before it has an initializer (`_ZGR` of a dynamically
        // initialized temporary): emission made that choice already.
        RelocModel::Static if !g.decl => Some(true),
        RelocModel::Pie(_) if !g.decl => None,
        _ => Some(
            direct_access
                && ((!g.function && !g.tls) || (g.function && reloc == RelocModel::Static)),
        ),
    }
}

impl Cg<'_> {
    /// The visibility of the global value of function or variable `d`
    /// (clang `setGlobalVisibility`): a definition takes `d`'s computed
    /// visibility, a declaration (`available_externally` included) only an
    /// explicit one.
    pub(crate) fn decl_visibility(&self, d: DeclId, definition: bool) -> Visibility {
        if !self.vis_active {
            return Visibility::Default;
        }
        let lv = self.lv(d, Comp::VALUE);
        if definition || lv.explicit { lv.vis } else { Visibility::Default }
    }

    /// The visibility of class `rd`'s vtable, VTT and `type_info` symbols
    /// (clang `setGVProperties(GV, RD)`: the class's type visibility).
    pub(crate) fn class_visibility(&self, rd: DeclId, definition: bool) -> Visibility {
        if !self.vis_active {
            return Visibility::Default;
        }
        let lv = self.lv(self.canon_tag(rd), Comp::TYPE);
        if definition || lv.explicit { lv.vis } else { Visibility::Default }
    }

    /// clang `Type::getVisibility`: the visibility of the `type_info`
    /// object and name of type `q` (`BuildTypeInfo`).
    pub(crate) fn type_visibility(&self, q: QualType) -> Visibility {
        if !self.vis_active {
            return Visibility::Default;
        }
        self.type_lv(q).vis
    }

    /// The definition of tag `d`, else its first declaration.
    fn canon_tag(&self, d: DeclId) -> DeclId {
        self.ast.definition(d).unwrap_or_else(|| self.ast.first(d))
    }

    /// clang `getLVForDecl` (visibility only; linkage is computed
    /// elsewhere and a local-linkage symbol is made default by
    /// [`Cg::finish_dso_local`]). Memoised; a cycle (a closure type
    /// naming its own owner variable) sees the default.
    fn lv(&self, d: DeclId, comp: Comp) -> Lv {
        let key = (d, comp.key());
        if let Some(&v) = self.lv_cache.borrow().get(&key) {
            return v;
        }
        self.lv_cache.borrow_mut().insert(key, Lv::DEFAULT);
        let v = self.compute_lv(d, comp);
        self.lv_cache.borrow_mut().insert(key, v);
        v
    }

    fn compute_lv(&self, d: DeclId, comp: Comp) -> Lv {
        let ast = self.ast;
        let mut c = ast.decls.get(d).ctx();
        while let Some(x) = c
            && matches!(ast.decls.get(x).kind(), DeclKind::LinkageSpec | DeclKind::Export)
        {
            c = ast.decls.get(x).ctx();
        }
        match c.map(|c| (c, ast.decls.get(c).kind())) {
            Some((c, k)) if is_record_kind(k) => self.member_lv(d, self.canon_tag(c), comp),
            Some((_, k)) if k.is_function() => self.local_lv(d, comp),
            _ => self.ns_lv(d, comp),
        }
    }

    /// The global `-fvisibility` mode (clang `getValueVisibilityMode` /
    /// `getTypeVisibilityMode`).
    fn global_vis(&self, comp: Comp) -> Visibility {
        if comp.ty { self.opts.type_visibility } else { self.opts.visibility }
    }

    /// clang `getLVForNamespaceScopeDecl`.
    fn ns_lv(&self, d: DeclId, comp: Comp) -> Lv {
        let ast = self.ast;
        let mut lv = Lv::DEFAULT;
        if !comp.ignore_explicit {
            if let Some(v) = self.explicit_vis(d, comp) {
                lv.merge_vis(v, true);
            } else {
                // The nearest enclosing namespace with an attribute (or a
                // `#pragma GCC visibility` around it) counts as explicit.
                let mut c = self.parent_ctx(d);
                while let Some(x) = c {
                    let k = ast.decls.get(x).kind();
                    if k == DeclKind::TranslationUnit {
                        break;
                    }
                    if k == DeclKind::Namespace
                        && let Some(v) = self.vis_of(x, comp.ty, false)
                    {
                        lv.merge_vis(v, true);
                        break;
                    }
                    c = self.parent_ctx(x);
                }
            }
            if !lv.explicit {
                lv.merge_vis(self.global_vis(comp), false);
                if self.use_inline_hidden(d) {
                    lv.merge_vis(Visibility::Hidden, false);
                }
            }
        }
        let first = ast.first(d);
        let k = ast.decls.get(d).kind();
        if is_var_kind(k) {
            if !lv.explicit
                && !kiln_abi::mangle::is_extern_c(ast, first)
                && let Some(t) = ast.decls.get(first).ty()
            {
                lv.merge(self.type_lv(t));
            }
            if let Some((_, s)) = ast.spec_info(first) {
                let consider = self.consider_class_template_vis(first, s.kind, comp);
                self.merge_args(&mut lv, s.args, consider, comp);
            }
        } else if k.is_function() {
            if let Some((_, s)) = ast.spec_info(first) {
                let consider = self.consider_fn_template_vis(first, s.kind);
                self.merge_args(&mut lv, s.args, consider, comp);
            }
        } else if is_record_kind(k)
            && let Some((_, s)) = ast.spec_info(first)
        {
            let consider = self.consider_class_template_vis(first, s.kind, comp);
            self.merge_args(&mut lv, s.args, consider, comp);
        }
        lv
    }

    /// clang `getLVForClassMember`: the member's own attribute (and
    /// `-fvisibility-inlines-hidden`), its template arguments, then its
    /// class's visibility.
    fn member_lv(&self, d: DeclId, class: DeclId, comp: Comp) -> Lv {
        let ast = self.ast;
        let mut lv = Lv::DEFAULT;
        if !comp.ignore_explicit {
            if let Some(v) = self.explicit_vis(d, comp) {
                lv.merge_vis(v, true);
            }
            if !lv.explicit && self.use_inline_hidden(d) {
                lv.merge_vis(Visibility::Hidden, false);
            }
        }
        // A member with its own attribute takes only its class's template
        // arguments' visibility, not the class's attribute.
        let class_comp = Comp { ignore_explicit: comp.ignore_explicit || lv.explicit, ..comp };
        let class_lv = self.lv(class, class_comp);
        let first = ast.first(d);
        let k = ast.decls.get(d).kind();
        let member_spec = |x: DeclId| {
            ast.member_spec_info(ast.first(x))
                .is_some_and(|(_, m)| m.kind == SpecKind::ExplicitSpecialization)
        };
        let mut suppressor = None;
        if k.is_function() {
            if let Some((_, s)) = ast.spec_info(first) {
                let consider = self.consider_fn_template_vis(first, s.kind);
                self.merge_args(&mut lv, s.args, consider, comp);
                if s.kind == SpecKind::ExplicitSpecialization {
                    suppressor = Some(first);
                } else if member_spec(s.template) {
                    suppressor = self.pattern_of(s.template);
                }
            } else if member_spec(first) {
                suppressor = Some(first);
            }
        } else if is_record_kind(k) {
            match ast.spec_info(first) {
                Some((_, s)) => {
                    let consider = self.consider_class_template_vis(first, s.kind, comp);
                    self.merge_args(&mut lv, s.args, consider, comp);
                    if s.kind == SpecKind::ExplicitSpecialization {
                        suppressor = Some(first);
                    } else if member_spec(s.template) {
                        suppressor = self.pattern_of(s.template);
                    }
                }
                _ if member_spec(first) => suppressor = Some(first),
                _ => {}
            }
        } else if is_var_kind(k) {
            if let Some((_, s)) = ast.spec_info(first) {
                let consider = self.consider_class_template_vis(first, s.kind, comp);
                self.merge_args(&mut lv, s.args, consider, comp);
            }
            if !lv.explicit
                && !class_lv.explicit
                && let Some(t) = ast.decls.get(first).ty()
            {
                lv.merge(self.type_lv(t));
            }
            if member_spec(first) {
                suppressor = Some(first);
            }
        }
        // An explicit (member) specialization with its own attribute
        // ignores its class's visibility.
        let consider_class = !suppressor.is_some_and(|s| {
            lv.explicit && class_lv.vis != Visibility::Default && self.has_direct_vis(s, comp)
        });
        if consider_class {
            lv.merge(class_lv);
        }
        lv
    }

    /// clang `getLVForLocalDecl`: a block-scope function declaration has
    /// only its own attribute; a static local, local class or closure of
    /// an inline function or template instantiation takes the function's
    /// visibility, except that a static local of a function hidden only
    /// by `-fvisibility-inlines-hidden` takes its class's explicit
    /// visibility, else the global mode.
    fn local_lv(&self, d: DeclId, comp: Comp) -> Lv {
        let ast = self.ast;
        let k = ast.decls.get(d).kind();
        let explicit_only = |s: &Self| {
            let mut lv = Lv::DEFAULT;
            if !comp.ignore_explicit
                && let Some(v) = s.explicit_vis(d, comp)
            {
                lv.merge_vis(v, true);
            }
            lv
        };
        if k.is_function() {
            return explicit_only(self);
        }
        if is_var_kind(k) {
            let ext = ast
                .decls
                .var(d)
                .is_some_and(|v| v.spec.storage() == kiln_ast::decl::VarStorage::Extern);
            if ext {
                return explicit_only(self);
            }
            if !crate::fold::is_static_local(ast, d) {
                return Lv::DEFAULT;
            }
        }
        // The outermost enclosing function.
        let mut outer = None;
        let mut c = ast.decls.get(d).ctx();
        while let Some(x) = c {
            let xk = ast.decls.get(x).kind();
            if xk.is_function() {
                outer = Some(x);
            }
            if matches!(xk, DeclKind::TranslationUnit) {
                break;
            }
            c = self.parent_ctx(x);
        }
        let Some(f) = outer else { return Lv::DEFAULT };
        let ffirst = ast.first(f);
        let tsk = ast
            .spec_info(ffirst)
            .map(|(_, s)| s.kind)
            .or_else(|| ast.member_spec_info(ffirst).map(|(_, m)| m.kind));
        let instantiation = matches!(
            tsk,
            Some(
                SpecKind::ImplicitInstantiation
                    | SpecKind::ExplicitInstantiationDeclaration
                    | SpecKind::ExplicitInstantiationDefinition
            )
        );
        if !self.fn_inlined(ffirst) && !instantiation {
            return Lv::DEFAULT;
        }
        let mut lv = self.lv(f, comp);
        if is_var_kind(k) && !lv.explicit && self.use_inline_hidden(f) {
            if let Some(cls) = ast.decls.get(f).ctx()
                && is_record_kind(ast.decls.get(cls).kind())
            {
                lv = self.lv(self.canon_tag(cls), comp);
            }
            if !lv.explicit {
                return Lv { vis: self.global_vis(comp), explicit: false };
            }
        }
        lv
    }

    /// The semantic context of `d` for the namespace walk: a namespace's
    /// is the one it is (re)opened in.
    fn parent_ctx(&self, d: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        if ast.decls.get(d).kind() == DeclKind::Namespace
            && let Some(l) = ast.decls.lexical_ctx(d)
        {
            return Some(l);
        }
        ast.decls.get(d).ctx()
    }

    /// The templated declaration of template `t`.
    fn pattern_of(&self, t: DeclId) -> Option<DeclId> {
        self.ast.template_info(t).and_then(|i| i.pattern)
    }

    /// clang `getVisibilityOf`: a `type_visibility` attribute (for type
    /// visibility), else a `visibility` attribute, on `d` or (`redecls`)
    /// any of its redeclarations.
    fn vis_of(&self, d: DeclId, ty: bool, redecls: bool) -> Option<Visibility> {
        let ast = self.ast;
        let on = |r: DeclId, k: AttrKind| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .map(|a| ast.attrs.get(a))
                .find(|a: &&AttrNode| a.kind == k)
                .map(|a| llvm_vis(a.visibility()))
        };
        let find = |k: AttrKind| {
            if redecls { ast.redecls_of(d).find_map(|r| on(r, k)) } else { on(d, k) }
        };
        if ty && let Some(v) = find(AttrKind::TYPE_VISIBILITY) {
            return Some(v);
        }
        find(AttrKind::VISIBILITY)
    }

    /// clang `hasDirectVisibilityAttribute`.
    fn has_direct_vis(&self, d: DeclId, comp: Comp) -> bool {
        (comp.ty && self.has_fn_attr(d, AttrKind::TYPE_VISIBILITY))
            || self.has_fn_attr(d, AttrKind::VISIBILITY)
    }

    /// clang `getExplicitVisibility`: `d`'s own attribute, else that of
    /// the pattern it was instantiated from (a member of a class template
    /// specialization, a class, variable or function template
    /// specialization, a template).
    fn explicit_vis(&self, d: DeclId, comp: Comp) -> Option<Visibility> {
        let ast = self.ast;
        let k = ast.decls.get(d).kind();
        if k == DeclKind::Namespace {
            return self.vis_of(d, comp.ty, false);
        }
        if let Some(v) = self.vis_of(d, comp.ty, true) {
            return Some(v);
        }
        let first = ast.first(d);
        let member = ast.member_spec_info(first).map(|(_, m)| m.instantiated_from);
        let spec = ast.spec_info(first).map(|(_, s)| s.template);
        if is_record_kind(k) || is_var_kind(k) || k.is_function() {
            if k.is_function()
                && let Some(t) = spec
            {
                return self.pattern_of(t).and_then(|p| self.vis_of(p, comp.ty, true));
            }
            if let Some(m) = member {
                return self.vis_of(m, comp.ty, true);
            }
            if let Some(t) = spec {
                return self.pattern_of(t).and_then(|p| self.vis_of(p, comp.ty, true));
            }
            // clang declares the replaceable global allocation functions
            // with an implicit `visibility("default")`
            // (`-fvisibility-global-new-delete=force-default`).
            if k == DeclKind::Function && self.alloc_fn(first).is_some_and(|a| a.replaceable) {
                return Some(Visibility::Default);
            }
            return None;
        }
        if kiln_ast::templates::is_template_kind(k) {
            return self.pattern_of(d).and_then(|p| self.vis_of(p, comp.ty, true));
        }
        None
    }

    /// clang `shouldConsiderTemplateVisibility` for a function template
    /// specialization: implicit, or without its own attribute.
    fn consider_fn_template_vis(&self, first: DeclId, kind: SpecKind) -> bool {
        !is_explicit_inst_or_spec(kind) || !self.has_fn_attr(first, AttrKind::VISIBILITY)
    }

    /// clang `shouldConsiderTemplateVisibility` for a class or variable
    /// template specialization.
    fn consider_class_template_vis(&self, first: DeclId, kind: SpecKind, comp: Comp) -> bool {
        if !is_explicit_inst_or_spec(kind) {
            return true;
        }
        if kind == SpecKind::ExplicitSpecialization && comp.ignore_explicit {
            return false;
        }
        !self.has_direct_vis(first, comp)
    }

    /// clang `mergeTemplateLV`'s argument part (template parameters'
    /// visibility is not modelled: spec/codegen.md "Symbol visibility").
    fn merge_args(&self, lv: &mut Lv, args: ArgListId, consider: bool, comp: Comp) {
        if consider {
            let a = self.args_lv(args, comp);
            lv.merge(a);
        }
    }

    /// clang `getLVForTemplateArgumentList`.
    fn args_lv(&self, list: ArgListId, comp: Comp) -> Lv {
        let ast = self.ast;
        let mut lv = Lv::DEFAULT;
        for a in ast.types.args(list) {
            match a.view() {
                ArgView::Type(q) => lv.merge(self.type_lv(q)),
                // A class-type argument is its template parameter object,
                // of its type's visibility (the entities its value names
                // are not merged).
                ArgView::Value { ty, .. } if ast.types.as_record(ty).is_some() => {
                    lv.merge(self.type_lv(ty));
                }
                ArgView::Value { value, .. } => match ast.consts.get(value) {
                    ConstValue::Scalar(Scalar::Pointer {
                        base: PtrBase::Decl(x) | PtrBase::Function(x),
                        ..
                    }) => lv.merge(self.lv(x, comp)),
                    ConstValue::Scalar(Scalar::MemberPtr { member, .. }) => {
                        lv.merge(self.lv(member, comp));
                    }
                    _ => {}
                },
                ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                    if let TemplateName::Template(t) =
                        ast.types.template_name_view(ast.types.canonical_template_name(n))
                    {
                        lv.merge(self.lv(t, comp));
                    }
                }
                ArgView::Pack(l) => lv.merge(self.args_lv(l, comp)),
                _ => {}
            }
        }
        lv
    }

    /// clang `getTypeLinkageAndVisibility` (visibility only): a class or
    /// enumeration's type visibility, through pointers, references,
    /// arrays, member pointers and function types.
    fn type_lv(&self, q: QualType) -> Lv {
        let ast = self.ast;
        let t = ast.types.canonical(q);
        match ast.types.canonical_view(t) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                self.type_lv(p)
            }
            TypeView::Complex(e) | TypeView::Atomic(e) => self.type_lv(e),
            TypeView::ConstantArray(_)
            | TypeView::IncompleteArray(_)
            | TypeView::VariableArray { .. }
            | TypeView::Vector { .. } => {
                ast.types.element(t).map_or(Lv::DEFAULT, |e| self.type_lv(e))
            }
            TypeView::MemberPointer { pointee, class } => {
                let mut lv = self.type_lv(QualType::unqual(class));
                lv.merge(self.type_lv(pointee));
                lv
            }
            TypeView::Function(p) => {
                let mut lv = self.type_lv(p.ret());
                for &x in ast.types.params(&p) {
                    lv.merge(self.type_lv(x));
                }
                lv
            }
            TypeView::Record(d) | TypeView::Enum(d) => self.lv(self.canon_tag(d), Comp::TYPE),
            _ => Lv::DEFAULT,
        }
    }

    /// clang `useInlineVisibilityHidden`: `-fvisibility-inlines-hidden`
    /// and `d` is an inline function that is not an explicit
    /// instantiation.
    fn use_inline_hidden(&self, d: DeclId) -> bool {
        let ast = self.ast;
        if !self.opts.inlines_hidden || !ast.decls.get(d).kind().is_function() {
            return false;
        }
        let first = ast.first(d);
        let own = ast
            .spec_info(first)
            .map(|(_, s)| s.kind)
            .or_else(|| ast.member_spec_info(first).map(|(_, m)| m.kind));
        let explicit_inst = |k: Option<SpecKind>| {
            matches!(
                k,
                Some(
                    SpecKind::ExplicitInstantiationDeclaration
                        | SpecKind::ExplicitInstantiationDefinition
                )
            )
        };
        if explicit_inst(own) || explicit_inst(self.explicit_inst_kind(first)) {
            return false;
        }
        self.fn_inlined(first)
    }

    /// clang `FunctionDecl::isInlined`: declared `inline` or `constexpr`
    /// or implicitly inline (defined in its class, an implicit member),
    /// on any declaration or on the pattern it was instantiated from.
    fn fn_inlined(&self, first: DeclId) -> bool {
        let ast = self.ast;
        let mut cur = Some(first);
        for _ in 0..8 {
            let Some(x) = cur else { return false };
            let inl = ast.redecls_of(x).any(|r| {
                ast.decls.function(r).is_some_and(|f| {
                    f.spec.is_inline()
                        || f.spec.implicit_inline()
                        || f.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None
                })
            });
            if inl {
                return true;
            }
            cur = ast.instantiated_from(ast.first(x)).map(|p| ast.first(p));
        }
        false
    }

    /// Whether non-TLS variable declarations are directly accessed
    /// (`-f[no-]direct-access-external-data`; on by default only for
    /// `-fno-pic`).
    pub(crate) fn direct_access_external_data(&self) -> bool {
        self.opts.direct_access_external_data.unwrap_or(self.opts.reloc == RelocModel::Static)
    }

    /// The last emission step: every global value of local linkage has
    /// default visibility (LLVM's verifier rule, clang
    /// `setGlobalVisibility`), and its `dso_local` follows the relocation
    /// model ([`assume_dso_local`]).
    pub(crate) fn finish_dso_local(&mut self) {
        let (reloc, si) = (self.opts.reloc, self.opts.semantic_interposition);
        let da = self.direct_access_external_data();
        finish_module(&mut self.m, reloc, si, da);
    }
}

fn finish_module(m: &mut Module, reloc: RelocModel, si: Option<bool>, da: bool) {
    let avail = |l: Linkage| l == Linkage::AvailableExternally;
    let fids: Vec<_> = m.functions().map(|(id, _)| id).collect();
    for id in fids {
        let f = m.func(id);
        let facts = GvFacts {
            name: f.name(),
            linkage: f.linkage,
            visibility: f.visibility,
            decl: f.is_declaration() || avail(f.linkage),
            function: true,
            tls: false,
            comdat: f.comdat.is_some(),
        };
        let d = assume_dso_local(&facts, reloc, si, da);
        let f = m.func_mut(id);
        if f.linkage.is_local() {
            f.visibility = Visibility::Default;
        }
        if let Some(d) = d {
            f.dso_local = d;
        }
    }
    let gids: Vec<_> = m.global_ids().collect();
    for id in gids {
        let g = m.global(id);
        let facts = GvFacts {
            name: g.name(),
            linkage: g.linkage,
            visibility: g.visibility,
            decl: g.initializer().is_none() || avail(g.linkage),
            function: false,
            tls: g.thread_local.is_some(),
            comdat: g.comdat.is_some(),
        };
        let d = assume_dso_local(&facts, reloc, si, da);
        let g = m.global_mut(id);
        if g.linkage.is_local() {
            g.visibility = Visibility::Default;
        }
        if let Some(d) = d {
            g.dso_local = d;
        }
    }
    let aids: Vec<_> = m.alias_ids().collect();
    for id in aids {
        let a = m.alias(id);
        let facts = GvFacts {
            name: a.name(),
            linkage: a.linkage,
            visibility: a.visibility,
            decl: false,
            function: false,
            tls: a.thread_local.is_some(),
            comdat: false,
        };
        let d = assume_dso_local(&facts, reloc, si, da);
        let a = m.alias_mut(id);
        if a.linkage.is_local() {
            a.visibility = Visibility::Default;
        }
        if let Some(d) = d {
            a.dso_local = d;
        }
    }
}
