//! Reflection literals (WP-501, spec/reflection.md §8.3): a
//! `std::meta::info` value mangles as its reflectee, structurally, in
//! GCC 16's `<reflection>` grammar under one of two `<info-type>`
//! spellings:
//!
//! ```text
//! <expr-primary> ::= L <info-type> <reflection> E
//! <info-type>    ::= u6__info      # kiln (default; a substitution candidate)
//!                ::= Dm            # gcc  (builtin; not a candidate)
//! ```
//!
//! The same reflection spells the same string in every translation unit,
//! so a specialization on one has its reflectee's linkage.

use std::fmt::Write as _;

use kiln_ast::Ast;
use kiln_ast::consts::{ConstValue, PrintEnv, PtrBase, Scalar, Shape};
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::{
    ArgListId, ConstValueId, DeclId, DmsId, ExprId, NnsId, QualType, ReflectionId,
};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{
    ArgView, BuiltinType, ExprOperand, Nns, TemplateArg, TemplateName, TypeFlags, TypeView,
};

use super::{
    Mangler, Structor, StructorSpelling, enclosing_function, has_internal_linkage, in_anon_ns,
    is_anon_ns, is_function_kind, is_internal_symbol, mangle_param_object, tag_externally_visible,
    type_externally_visible, value_externally_visible,
};

/// How a `std::meta::info` value mangles (`-fkiln-reflection-mangling=`,
/// spec/reflection.md §8.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReflectionMangling {
    /// `L u6__info <reflection> E`: GCC's `<reflection>` under kiln's
    /// vendor type (opt-in, experimental).
    Kiln,
    /// `L Dm <reflection> E`, link-compatible with g++ 16 (the default,
    /// WP-909).
    #[default]
    Gcc,
    /// Prototype (hidden): `L u6__info h <32 hex digits> E`, a 128-bit
    /// hash of the `kiln` form's `<reflection>`.
    KilnHashed,
}

impl ReflectionMangling {
    /// The form named `s` (`kiln`, `gcc`, or the hidden `kiln-hashed`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "kiln" => Some(Self::Kiln),
            "gcc" => Some(Self::Gcc),
            "kiln-hashed" => Some(Self::KilnHashed),
            _ => None,
        }
    }
}

/// Whether reflection `id` mangles as an externally visible entity: the
/// linkage of its reflectee ([basic.link]/15: a reflection of an entity
/// without linkage is TU-local). An annotation has no cross-TU spelling
/// and is internal.
pub(super) fn reflection_externally_visible(ast: &Ast, id: ReflectionId) -> bool {
    match ast.reflections.get(id) {
        Reflectee::Null => true,
        Reflectee::Type(q) => type_externally_visible(ast, q) && alias_visible(ast, q),
        Reflectee::Decl(d) => decl_visible(ast, d),
        Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
            !is_anon_ns(ast, d) && !in_anon_ns(ast, d) && enclosing_function(ast, d).is_none()
        }
        Reflectee::Template(n) => match ast.types.template_name_view(n) {
            TemplateName::Template(t) => !has_internal_linkage(ast, t),
            _ => true,
        },
        Reflectee::Value(v) | Reflectee::Object(v) => value_externally_visible(ast, v),
        Reflectee::BaseSpec(d, _) => tag_externally_visible(ast, ast.first(d)),
        Reflectee::FnParam(f, _) => !is_internal_symbol(ast, f),
        // Its type's and annotations' linkage, once described.
        Reflectee::DataMemberSpec(s) => ast.reflections.dms(s).is_some_and(|d| {
            type_externally_visible(ast, d.ty)
                && d.annotations.iter().all(|&a| reflection_externally_visible(ast, a))
        }),
        // GCC's annotation index is TU-local.
        Reflectee::Annotation(_) => false,
    }
}

/// A kept alias (`TypeView::Typedef`) declared in a function or an
/// unnamed namespace is TU-local.
fn alias_visible(ast: &Ast, q: QualType) -> bool {
    match ast.types.view(q.ty()) {
        TypeView::Typedef { decl, .. } => {
            enclosing_function(ast, decl).is_none() && !in_anon_ns(ast, decl)
        }
        _ => true,
    }
}

fn decl_visible(ast: &Ast, d: DeclId) -> bool {
    match ast.decls.get(d).kind() {
        // A member of a class or enumeration has the class's linkage.
        DeclKind::Field | DeclKind::IndirectField | DeclKind::EnumConstant => {
            match ast.decls.get(d).ctx() {
                Some(c)
                    if matches!(ast.decls.get(c).kind(), DeclKind::Enum) || is_record(ast, c) =>
                {
                    tag_externally_visible(ast, ast.first(c))
                }
                _ => false,
            }
        }
        DeclKind::ParmVar => false,
        _ => !is_internal_symbol(ast, d),
    }
}

fn is_record(ast: &Ast, d: DeclId) -> bool {
    super::is_record_kind(ast.decls.get(d).kind())
}

/// Whether the arguments of every specialization of alias template `t`
/// are determined by the type it aliases: each template parameter is
/// deducible ([temp.deduct.type]/8) from the defining-type-id. Then GCC's
/// `ta` form, which writes the aliased specialization's arguments
/// instead of the alias's, is injective and gcc mode can use it.
/// Conservative: a form not recognized as deducible makes it `false`.
pub(super) fn alias_args_recoverable(ast: &Ast, t: DeclId) -> bool {
    let Some(info) = ast.template_info(t) else { return false };
    let list = *ast.templates.param_list(info.params);
    let params: Vec<DeclId> = ast.lists.iter::<DeclId>(list.params).collect();
    if params.is_empty() {
        return true;
    }
    let Some(rhs) = info.pattern.and_then(|p| ast.decls.typedef(p)).map(|t| t.underlying) else {
        return false;
    };
    let mut seen = vec![false; params.len()];
    deducible_params(ast, rhs, list.depth, &mut seen, 0);
    seen.iter().all(|&b| b)
}

/// Marks in `seen` the parameters at `depth` that `q` names in a deduced
/// context ([temp.deduct.type]/8): the type itself, compound types built
/// from deduced parts, and a class template specialization's type,
/// template and bare non-type arguments.
fn deducible_params(ast: &Ast, q: QualType, depth: u16, seen: &mut [bool], level: u32) {
    if level > 64 {
        return;
    }
    let mark = |d: DeclId, seen: &mut [bool]| {
        if let Some(p) = ast.template_parm(d)
            && p.depth == depth
            && let Some(s) = seen.get_mut(usize::from(p.index))
        {
            *s = true;
        }
    };
    let rec = |q: QualType, seen: &mut [bool]| deducible_params(ast, q, depth, seen, level + 1);
    match ast.types.view(q.ty()) {
        TypeView::TemplateTypeParm(d) => mark(d, seen),
        TypeView::CanonTemplateTypeParm { depth: d, index, .. } => {
            if d == u32::from(depth)
                && let Some(s) = usize::try_from(index).ok().and_then(|i| seen.get_mut(i))
            {
                *s = true;
            }
        }
        TypeView::Pointer(x)
        | TypeView::LValueRef(x)
        | TypeView::RValueRef(x)
        | TypeView::Paren(x)
        | TypeView::IncompleteArray(x)
        | TypeView::Typedef { underlying: x, .. }
        | TypeView::Using { underlying: x, .. }
        | TypeView::Elaborated { named: x, .. }
        | TypeView::SubstTemplateTypeParm { replacement: x, .. }
        | TypeView::PackExpansion { pattern: x, .. }
        | TypeView::ExtQual { base: x, .. } => rec(x, seen),
        TypeView::ConstantArray(a) => rec(a.elem, seen),
        TypeView::DependentSizedArray { elem, size } => {
            rec(elem, seen);
            if let ExprOperand::Expr(e) = size {
                deducible_nttp(ast, e, depth, seen);
            }
        }
        TypeView::MemberPointer { pointee, class } => {
            rec(pointee, seen);
            rec(QualType::unqual(class), seen);
        }
        TypeView::Function(p) => {
            rec(p.ret(), seen);
            for &x in ast.types.params(&p) {
                rec(x, seen);
            }
        }
        // An alias's own arguments are deducible only through an alias
        // that is itself recoverable.
        TypeView::TemplateSpecialization { name, args, alias, .. } => {
            if alias
                && !ast.types.template_decl(name).is_some_and(|t| alias_args_recoverable(ast, t))
            {
                return;
            }
            if let TemplateName::Template(d) = ast.types.template_name_view(name) {
                mark(d, seen);
            }
            deducible_args(ast, args, depth, seen, level + 1);
        }
        _ => {}
    }
}

fn deducible_args(ast: &Ast, args: ArgListId, depth: u16, seen: &mut [bool], level: u32) {
    for a in ast.types.args(args) {
        match a.view() {
            ArgView::Type(x) => deducible_params(ast, x, depth, seen, level),
            ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                if let TemplateName::Template(d) = ast.types.template_name_view(n)
                    && let Some(p) = ast.template_parm(d)
                    && p.depth == depth
                    && let Some(s) = seen.get_mut(usize::from(p.index))
                {
                    *s = true;
                }
            }
            ArgView::Expr { expr, .. } => deducible_nttp(ast, expr, depth, seen),
            ArgView::Pack(l) => deducible_args(ast, l, depth, seen, level),
            _ => {}
        }
    }
}

/// Marks the non-type template parameter at `depth` that `e` names bare
/// (through parentheses, implicit conversions and a pack expansion): a
/// deduced context ([temp.deduct.type]/8, `TT<i>` and `T[i]`).
fn deducible_nttp(ast: &Ast, e: ExprId, depth: u16, seen: &mut [bool]) {
    let mut e = e;
    while matches!(
        ast.exprs.get(e).kind(),
        ExprKind::ParenExpr | ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr
    ) {
        e = ast.exprs.operand(e);
    }
    if ast.exprs.get(e).kind() == ExprKind::PackExpansionExpr {
        e = ast.exprs.operand(e);
    }
    if ast.exprs.get(e).kind() == ExprKind::DeclRefExpr {
        let d = ast.exprs.decl(e);
        if ast.decls.get(d).kind() == DeclKind::NonTypeTemplateParm
            && let Some(p) = ast.template_parm(d)
            && p.depth == depth
            && let Some(s) = seen.get_mut(usize::from(p.index))
        {
            *s = true;
        }
    }
}

/// Whether the specialization of alias template `t` with arguments
/// `args` keeps every argument in its aliased type (WP-1260): each
/// parameter is deducible from the defining-type-id
/// ([temp.deduct.type]/8), or appears in a nested-name-specifier
/// ([temp.deduct.type]/5.1, a non-deduced context) that names a
/// parameter whose argument is dependent. Such a qualifier stays a
/// dependent name after substitution ([temp.dep.type]/7), written
/// structurally, so the aliased type determines `args` and GCC's `ta`
/// form (the aliased type, no alias arguments) is injective for it:
/// `^^D<T>` with `D = typename RC<T>::type`. A qualifier whose
/// parameters all take non-dependent arguments names a member of a
/// concrete specialization and loses them (`^^D<int>` and
/// `^^D<const int>` alias one `int`). A pack argument counts as
/// dependent when it is non-empty and every element is. Conservative,
/// as [`alias_args_recoverable`]: other non-deduced contexts (a
/// `decltype`, a non-bare expression) recover nothing.
pub(super) fn alias_args_preserved(ast: &Ast, t: DeclId, args: ArgListId) -> bool {
    let Some(info) = ast.template_info(t) else { return false };
    let list = *ast.templates.param_list(info.params);
    let params: Vec<DeclId> = ast.lists.iter::<DeclId>(list.params).collect();
    let given = ast.types.args(args);
    if params.is_empty() || given.len() != params.len() {
        return false;
    }
    let Some(rhs) = info.pattern.and_then(|p| ast.decls.typedef(p)).map(|t| t.underlying) else {
        return false;
    };
    let dependent = |a: TemplateArg| ast.types.arg_flags(a).intersects(TypeFlags::DEPENDENT);
    let dep: Vec<bool> = given
        .iter()
        .map(|&a| match a.view() {
            ArgView::Pack(l) => {
                let elems = ast.types.args(l);
                !elems.is_empty() && elems.iter().all(|&e| dependent(e))
            }
            _ => dependent(a),
        })
        .collect();
    let mut seen = vec![false; params.len()];
    deducible_params(ast, rhs, list.depth, &mut seen, 0);
    preserved_params(ast, rhs, list.depth, &dep, &mut seen, 0);
    seen.iter().all(|&b| b)
}

/// Marks in `seen` the parameters at `depth` that `q` names inside a
/// dependent name's qualifier or a `decltype` operand that substitution
/// keeps (see [`alias_args_preserved`]); `dep` says which parameters
/// take a dependent argument. An alias template-id is looked through to
/// its aliased type, which is what the mangling writes.
fn preserved_params(
    ast: &Ast,
    q: QualType,
    depth: u16,
    dep: &[bool],
    seen: &mut [bool],
    level: u32,
) {
    if level > 64 {
        return;
    }
    let rec =
        |q: QualType, seen: &mut [bool]| preserved_params(ast, q, depth, dep, seen, level + 1);
    // The parameters `inner` names, marked only when one of them takes a
    // dependent argument: the qualifier or operand then stays dependent
    // and is written structurally.
    let keep = |inner: Vec<bool>, seen: &mut [bool]| {
        if inner.iter().zip(dep).any(|(&s, &d)| s && d) {
            for (s, i) in seen.iter_mut().zip(inner) {
                *s |= i;
            }
        }
    };
    let decltype = |e: ExprId, seen: &mut [bool]| {
        let mut inner = vec![false; seen.len()];
        expr_params(ast, e, depth, dep, &mut inner, level + 1);
        keep(inner, seen);
    };
    match ast.types.view(q.ty()) {
        TypeView::DependentName { nns, .. } => {
            let mut inner = vec![false; seen.len()];
            qualifier_params(ast, nns, depth, dep, &mut inner, level + 1);
            keep(inner, seen);
        }
        // `Q::template X<A>`: `A` is kept when `Q` is.
        TypeView::DependentTemplateSpecialization { nns, args, .. } => {
            let mut inner = vec![false; seen.len()];
            qualifier_params(ast, nns, depth, dep, &mut inner, level + 1);
            if inner.iter().zip(dep).any(|(&s, &d)| s && d) {
                deducible_args(ast, args, depth, &mut inner, level + 1);
                for a in ast.types.args(args) {
                    if let ArgView::Type(x) = a.view() {
                        preserved_params(ast, x, depth, dep, &mut inner, level + 1);
                    }
                }
                keep(inner, seen);
            }
        }
        TypeView::Decltype { expr, .. } => decltype(expr, seen),
        TypeView::DependentDecltype(p) => {
            if let Some(e) = ast.types.profile_expr(p) {
                decltype(e, seen);
            }
        }
        TypeView::Pointer(x)
        | TypeView::LValueRef(x)
        | TypeView::RValueRef(x)
        | TypeView::Paren(x)
        | TypeView::IncompleteArray(x)
        | TypeView::Typedef { underlying: x, .. }
        | TypeView::Using { underlying: x, .. }
        | TypeView::Elaborated { named: x, .. }
        | TypeView::SubstTemplateTypeParm { replacement: x, .. }
        | TypeView::PackExpansion { pattern: x, .. }
        | TypeView::ExtQual { base: x, .. } => rec(x, seen),
        TypeView::ConstantArray(a) => rec(a.elem, seen),
        TypeView::DependentSizedArray { elem, .. } => rec(elem, seen),
        TypeView::MemberPointer { pointee, class } => {
            rec(pointee, seen);
            rec(QualType::unqual(class), seen);
        }
        TypeView::Function(p) => {
            rec(p.ret(), seen);
            for &x in ast.types.params(&p) {
                rec(x, seen);
            }
        }
        TypeView::TemplateSpecialization { underlying: Some(u), alias: true, .. } => rec(u, seen),
        TypeView::TemplateSpecialization { args, alias: false, .. } => {
            for a in ast.types.args(args) {
                match a.view() {
                    ArgView::Type(x) => rec(x, seen),
                    ArgView::Pack(l) => {
                        for b in ast.types.args(l) {
                            if let ArgView::Type(x) = b.view() {
                                rec(x, seen);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// The parameters at `depth` that the type components of qualifier `n`
/// (and its prefixes) name in a kept position.
fn qualifier_params(ast: &Ast, n: NnsId, depth: u16, dep: &[bool], seen: &mut [bool], level: u32) {
    let mut n = Some(n);
    let mut level = level;
    while let Some(id) = n {
        if level > 64 {
            return;
        }
        n = match ast.types.nns_view(id) {
            Nns::Type { prefix, ty, .. } => {
                deducible_params(ast, ty, depth, seen, level);
                preserved_params(ast, ty, depth, dep, seen, level);
                prefix
            }
            Nns::Identifier { prefix, .. } => prefix,
            _ => None,
        };
        level += 1;
    }
}

/// The parameters at `depth` that expression `e` names (a `decltype`
/// operand, written as an expression in the mangling): a bare non-type
/// parameter, and the parameters of each subexpression's type (and a
/// `sizeof`/`alignof` type operand) in a kept position. Bounded: a larger operand recovers nothing more.
fn expr_params(ast: &Ast, e: ExprId, depth: u16, dep: &[bool], seen: &mut [bool], level: u32) {
    let mut stack = vec![e];
    let mut budget = 4096u32;
    while let Some(x) = stack.pop() {
        budget = match budget.checked_sub(1) {
            Some(b) => b,
            None => return,
        };
        deducible_nttp(ast, x, depth, seen);
        let mut types = vec![ast.exprs.get(x).ty()];
        if ast.exprs.get(x).kind() == ExprKind::UnaryExprOrTypeTraitExpr {
            types.extend(ast.exprs.trait_type_arg(x));
        }
        for ty in types {
            deducible_params(ast, ty, depth, seen, level);
            preserved_params(ast, ty, depth, dep, seen, level);
        }
        stack.extend(ast.exprs.children(x, &ast.lists));
    }
}

/// The FNV-1a 128-bit hash of `s` (the `kiln-hashed` prototype).
fn fnv1a_128(s: &str) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    s.bytes().fold(OFFSET, |h, b| (h ^ u128::from(b)).wrapping_mul(PRIME))
}

impl Mangler<'_, '_> {
    fn gcc(&self) -> bool {
        self.anon.refl_mode == ReflectionMangling::Gcc
    }

    /// `L <info-type> <reflection> E` for reflection `id` (`None`: the
    /// null reflection) of type `ty`.
    pub(super) fn reflection_literal(&mut self, ty: QualType, id: Option<ReflectionId>) {
        self.out.push('L');
        self.ty(ty);
        let r = id.map_or(Reflectee::Null, |id| self.ast.reflections.get(id));
        if self.anon.refl_mode == ReflectionMangling::KilnHashed {
            // The structural form in a fresh substitution context, so the
            // hash is the reflectee's alone.
            let body = {
                let mut m = Mangler::new(self.ast, &mut *self.anon, "");
                m.reflection(r);
                m.out
            };
            let _ = write!(self.out, "h{:032x}", fnv1a_128(&body));
        } else {
            self.reflection(r);
        }
        self.out.push('E');
        self.anon.refl_literals += 1;
    }

    /// `<compact-number>`: `_` for 0, `<n-1>_` otherwise.
    fn compact(&mut self, n: u32) {
        if n > 0 {
            let _ = write!(self.out, "{}", n - 1);
        }
        self.out.push('_');
    }

    /// `<prefix>` of context `dc` as GCC's `write_prefix`: nothing for the
    /// translation unit, `St` for `std`; a context in a function is
    /// written `Z <encoding> E <prefix>` (kiln).
    fn refl_prefix(&mut self, dc: Option<DeclId>) {
        let Some(dc) = dc else { return };
        let ast = self.ast;
        let f = if is_function_kind(ast.decls.get(dc).kind()) {
            Some(dc)
        } else {
            enclosing_function(ast, dc)
        };
        if let Some(f) = f {
            self.out.push('Z');
            self.encoding(f, Structor::Complete);
            self.out.push('E');
            if f != dc {
                self.prefix(dc, Some(f));
            }
        } else {
            self.prefix(dc, None);
        }
    }

    /// The context of member `d`, looking through anonymous structs and
    /// unions (GCC's `dm`).
    fn member_ctx(&self, d: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        let mut c = ast.decls.get(d).ctx();
        while let Some(x) = c
            && ast.decls.record(x).is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS))
        {
            c = ast.decls.get(x).ctx();
        }
        c
    }

    /// `<reflection>` of `r` (GCC `write_reflection`).
    pub(super) fn reflection(&mut self, r: Reflectee) {
        let ast = self.ast;
        match r {
            Reflectee::Null => self.out.push_str("nu"),
            Reflectee::Type(q) => self.reflected_type(q),
            Reflectee::Decl(d) => self.reflected_decl(d),
            Reflectee::Namespace(d) if ast.decls.get(d).kind() == DeclKind::TranslationUnit => {
                self.out.push_str("gs");
            }
            Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.out.push_str(if matches!(r, Reflectee::Namespace(_)) { "ns" } else { "na" });
                let p = self.parent(d);
                self.refl_prefix(p);
                self.unqualified(d, Structor::Complete);
            }
            Reflectee::Template(n) => {
                let n = ast.types.canonical_template_name(n);
                if let TemplateName::Template(t) = ast.types.template_name_view(n) {
                    self.out.push_str(match ast.decls.get(t).kind() {
                        DeclKind::FunctionTemplate => "ft",
                        DeclKind::VarTemplate => "vt",
                        DeclKind::TypeAliasTemplate => "at",
                        DeclKind::Concept => "co",
                        _ => "ct",
                    });
                    let p = self.parent(t);
                    self.refl_prefix(p);
                    self.unqualified(t, Structor::Complete);
                } else {
                    self.out.push_str("tt");
                    self.template_name(n);
                }
            }
            Reflectee::Value(v) => {
                self.out.push_str("vl");
                let ty = ast.consts.ty(v);
                match ast.consts.get(v) {
                    ConstValue::Scalar(Scalar::Pointer { .. }) => self.refl_pointer(v, true),
                    // `nullptr` is GCC's `LDnE` (clang's argument form is
                    // `LDn0E`).
                    _ if ast.types.builtin(ast.types.canonical(ty))
                        == Some(BuiltinType::NullPtr) =>
                    {
                        self.out.push_str("LDnE");
                    }
                    _ => self.value_in_arg(ty, v, false),
                }
            }
            Reflectee::Object(v) => {
                self.out.push_str("ob");
                self.refl_pointer(v, false);
            }
            Reflectee::BaseSpec(d, i) => {
                self.out.push_str("ba");
                self.compact(i);
                self.tag(d);
            }
            Reflectee::FnParam(f, i) => {
                self.out.push_str("pa");
                self.compact(i);
                self.encoding(f, Structor::Complete);
            }
            Reflectee::Annotation(a) => {
                self.out.push_str("an");
                self.compact(a.index() - 1);
            }
            Reflectee::DataMemberSpec(s) => self.data_member_spec(s),
        }
    }

    /// `ds <type> _ [<source-name>] _ [<alignment>] _ [<width>] _ [n]
    /// <template-arg>*` (one per annotation); `ds _ <id> _` (TU-local)
    /// when codegen recorded no description.
    fn data_member_spec(&mut self, s: DmsId) {
        let ast = self.ast;
        let Some(d) = ast.reflections.dms(s) else {
            let _ = write!(self.out, "ds_{}_", s.index());
            return;
        };
        self.out.push_str("ds");
        self.ty(d.ty);
        self.out.push('_');
        if let Some(n) = d.name {
            self.source_name(ast.symbols().as_str(n));
        }
        self.out.push('_');
        if let Some(a) = d.alignment {
            let _ = write!(self.out, "{a}");
        }
        self.out.push('_');
        if let Some(w) = d.bit_width {
            let _ = write!(self.out, "{w}");
        }
        self.out.push('_');
        if d.nua {
            self.out.push('n');
        }
        for &a in &d.annotations {
            // GCC writes each annotation's constant as a template
            // argument.
            match ast.reflections.get(a) {
                Reflectee::Value(v) => self.value_arg(ast.consts.ty(v), v),
                r => self.reflection(r),
            }
        }
    }

    /// `ta <prefix> <unqualified> [<template-args>] _ <type>` for a kept
    /// alias, else `ty <type>`.
    fn reflected_type(&mut self, q: QualType) {
        let ast = self.ast;
        if q.quals_bits() == 0 {
            match ast.types.view(q.ty()) {
                TypeView::Typedef { decl, underlying } => {
                    self.out.push_str("ta");
                    let p = self.parent(decl);
                    self.refl_prefix(p);
                    self.unqualified(decl, Structor::Complete);
                    // GCC writes the arguments of an aliased class
                    // template specialization (`maybe_template_info` of
                    // the alias); they are also in the type, so kiln
                    // does not.
                    if self.gcc() {
                        self.underlying_spec_args(underlying);
                    }
                    self.out.push('_');
                    self.ty(underlying);
                    return;
                }
                TypeView::TemplateSpecialization {
                    name,
                    args,
                    underlying: Some(u),
                    alias: true,
                } => {
                    if let TemplateName::Template(t) =
                        ast.types.template_name_view(ast.types.canonical_template_name(name))
                    {
                        self.out.push_str("ta");
                        let p = self.parent(t);
                        self.refl_prefix(p);
                        self.unqualified(t, Structor::Complete);
                        // GCC writes the arguments of the aliased class
                        // template specialization, not the alias's. That
                        // loses the alias's arguments when the aliased type
                        // does not determine them (`template <class T, class
                        // U> using F = C<T>`: GCC's `^^F<T, int>` and
                        // `^^F<T, long>` collide), so gcc mode writes GCC's
                        // form only for an alias whose every parameter is
                        // deducible from its defining-type-id
                        // (`alias_args_recoverable`) or kept in a
                        // dependent qualifier for these arguments
                        // (`alias_args_preserved`, WP-1260: `^^D<T>`, `D =
                        // typename RC<T>::type`), and kiln's (the alias's
                        // own arguments) otherwise (WP-1250).
                        let gcc_form = self.gcc()
                            && (alias_args_recoverable(ast, t)
                                || alias_args_preserved(ast, t, args));
                        if gcc_form {
                            self.underlying_spec_args(u);
                        } else {
                            self.template_args(args);
                        }
                        self.out.push('_');
                        self.ty(u);
                        return;
                    }
                }
                _ => {}
            }
        }
        self.out.push_str("ty");
        self.ty(q);
    }

    /// The template arguments of the class template specialization `u`
    /// aliases (GCC's `maybe_template_info` of an alias), for a
    /// specialization (`Record`) or a dependent one
    /// (`TemplateSpecialization`).
    fn underlying_spec_args(&mut self, u: QualType) {
        let ast = self.ast;
        match ast.types.canonical_view(ast.types.canonical(u)) {
            TypeView::Record(r) => {
                if let Some((_, args)) = self.spec_of(r) {
                    self.template_args(args);
                }
            }
            TypeView::TemplateSpecialization { args, alias: false, .. } => self.template_args(args),
            _ => {}
        }
    }

    fn reflected_decl(&mut self, d: DeclId) {
        let ast = self.ast;
        match ast.decls.get(d).kind() {
            DeclKind::Binding => {
                self.out.push_str("sb");
                self.name(d, Structor::Complete);
            }
            k if is_function_kind(k) => {
                self.out.push_str("fn");
                let saved = self.structors;
                self.structors = if matches!(k, DeclKind::CXXConstructor | DeclKind::CXXDestructor)
                {
                    StructorSpelling::Unified
                } else if saved == StructorSpelling::Unified {
                    StructorSpelling::Itanium
                } else {
                    saved
                };
                self.encoding(d, Structor::Complete);
                self.structors = saved;
            }
            DeclKind::EnumConstant => {
                self.out.push_str("en");
                let p = ast.decls.get(d).ctx();
                self.refl_prefix(p);
                self.unqualified(d, Structor::Complete);
            }
            DeclKind::Field | DeclKind::IndirectField => {
                let unnamed_bitfield = ast.decls.get(d).name().is_empty()
                    && ast.decls.field(d).is_some_and(|f| f.bit_width.is_some());
                if unnamed_bitfield {
                    // `un <prefix> <index among the unnamed bit-fields>`.
                    self.out.push_str("un");
                    let c = ast.decls.get(d).ctx();
                    let idx = c.map_or(0, |c| {
                        ast.members(c)
                            .take_while(|&m| m != d)
                            .filter(|&m| {
                                ast.decls.get(m).kind() == DeclKind::Field
                                    && ast.decls.get(m).name().is_empty()
                                    && ast.decls.field(m).is_some_and(|f| f.bit_width.is_some())
                            })
                            .count()
                    });
                    self.refl_prefix(c);
                    self.compact(u32::try_from(idx).unwrap_or(u32::MAX));
                } else {
                    self.out.push_str("dm");
                    let c = self.member_ctx(d);
                    self.refl_prefix(c);
                    self.unqualified(d, Structor::Complete);
                }
            }
            _ => {
                self.out.push_str("vr");
                self.name(d, Structor::Complete);
            }
        }
    }

    /// A pointer value (`addr`: `vl`, its address) or the object it
    /// points to (`ob`): an element of a declared array is GCC's
    /// `ix <array> <index>`; a template parameter object is the
    /// external name `L _Z <length> _ZTA... E` (GCC spells its mangled
    /// name as a source name); else the template-argument form without
    /// its `X`.
    fn refl_pointer(&mut self, v: ConstValueId, addr: bool) {
        let ast = self.ast;
        let ty = ast.consts.ty(v);
        let ConstValue::Scalar(Scalar::Pointer { base, offset, designated, .. }) =
            ast.consts.get(v)
        else {
            self.value_in_arg(ty, v, false);
            return;
        };
        let is_ref =
            matches!(ast.types.canonical_view(ty), TypeView::LValueRef(_) | TypeView::RValueRef(_));
        let addr = addr && !is_ref;
        if let PtrBase::Decl(d) = base {
            if ast.decls.get(d).kind() == DeclKind::TemplateParamObject && offset == 0 {
                if addr {
                    self.out.push_str("ad");
                }
                let s = mangle_param_object(ast, &mut *self.anon, d);
                let _ = write!(self.out, "L_Z{}{s}E", s.len());
                return;
            }
            if let Some(i) =
                self.array_index(d, offset, designated.or_else(|| ast.types.pointee(ty)))
            {
                if addr {
                    self.out.push_str("ad");
                }
                self.out.push_str("ixL");
                self.entity(d);
                self.out.push_str("EL");
                self.ty(QualType::unqual(ast.types.target().ptrdiff_t));
                let _ = write!(self.out, "{i}E");
                return;
            }
        }
        let at = self.out.len();
        self.pointer_value(ty, base, offset, designated, false);
        if !addr && !is_ref && self.out[at..].starts_with("ad") {
            self.out.replace_range(at..at + 2, "");
        }
    }

    /// The index of the element of array variable `d` at byte `offset`,
    /// when the pointee is that element.
    fn array_index(&self, d: DeclId, offset: u64, pointee: Option<QualType>) -> Option<u64> {
        let ast = self.ast;
        let Shape::Array { elem, stride, len } = ast.shape(ast.decls.get(d).ty()?) else {
            return None;
        };
        let same = ast.types.canonical(pointee?).without_quals()
            == ast.types.canonical(elem).without_quals();
        (same && stride != 0 && offset.is_multiple_of(stride) && offset / stride < len)
            .then_some(offset / stride)
    }

    /// A `CXXReflectExpr` in a dependent template argument or signature
    /// (`K<^^T>`): `L <info-type> ty <type> E`, `L <info-type> de
    /// <expression> E` for a dependent name, else the reflectee. `false`
    /// when the operand has no encoding.
    pub(super) fn reflect_expr(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        self.out.push('L');
        self.ty(ast.exprs.get(e).ty());
        if let Some(name) = ast.exprs.reflect_dependent_name(e) {
            self.out.push_str("de");
            if !self.expression(name) {
                return false;
            }
        } else {
            match ast.reflect_operand(e) {
                // A dependent alias template specialization keeps its
                // alias (`ta`, as g++-16 writes it): `^^A<T>` and the
                // type it aliases are different reflections (WP-1250).
                Reflectee::Type(q) if ast.types.is_dependent(q) => self.reflected_type(q),
                r => self.reflection(r),
            }
        }
        self.out.push('E');
        self.anon.refl_literals += 1;
        true
    }
}
