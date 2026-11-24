//! clang `PredefinedExpr::ComputeName(PrettyFunction, f)`: the
//! `__PRETTY_FUNCTION__` spelling of a function, shared by Sema (the
//! predefined identifiers) and the `-fdump-vtable-layouts` printer
//! (`PrettyFunctionNoVirtual`).

use crate::Ast;
use crate::consts::{PrintOptions as ValueOptions, PrintStyle};
use crate::decl::{DeclKind, DefinitionBits, FnStorage};
use crate::fn_names::{CLASS_NAME, FnNameRole};
use crate::ids::{DeclId, QualType};
use crate::templates::SpecKind;
use crate::types::{
    ArgView, FuncProto, PrintOptions, RefQual, TemplateArg, TypeNames, TypeView, write_declarator,
    write_template_arg, write_type,
};

use super::AstNames;

impl Ast {
    /// The instantiation pattern of function `f` (clang
    /// `getTemplateInstantiationPattern`): the declaration its definition
    /// is instantiated from, through member templates; `f` itself for an
    /// explicit specialization (also of a member of a class template
    /// specialization, `template<> int C<long>::m(long)`) or a
    /// non-template.
    pub fn function_pattern(&self, f: DeclId) -> DeclId {
        let mut cur = f;
        for _ in 0..64 {
            let explicit = SpecKind::ExplicitSpecialization;
            if self.spec_info(cur).is_some_and(|(_, s)| s.kind == explicit)
                || self.member_spec_info(cur).is_some_and(|(_, s)| s.kind == explicit)
            {
                break;
            }
            match self.instantiated_from(cur) {
                Some(p) if p != cur && self.decls.get(p).kind().is_function() => cur = p,
                _ => break,
            }
        }
        cur
    }
}

/// The declared function type of `f`, with its sugar.
fn sugared_proto(ast: &Ast, f: DeclId) -> Option<&FuncProto> {
    let q = ast.decls.get(f).ty()?;
    let q = ast.types.desugar_top(q);
    ast.types.proto_of(q.ty()).or_else(|| ast.types.proto(q))
}

/// clang `PredefinedExpr::ComputeName(PrettyFunction, f)` (with
/// `virtual_static`) or `ComputeName(PrettyFunctionNoVirtual, f)`
/// (without: no `virtual ` / `static ` prefix): `int A<int>::f(T) const
/// [T = int]`. The parameter and return types are the pattern's, as
/// written; the bracketed list names the arguments of the enclosing
/// implicit class template specializations and of `f` itself. `names`
/// sets the scope printing (unnamed tags, closures, scoped arguments).
///
/// With a span recorder on `names` (spec/ast.md §24.3), every name is
/// written with its role; the parameter and return types then print
/// through a recording stand-in for the bare [`Ast`]'s names
/// ([`AstNames::plain_ast`]), which prints them alike.
pub fn pretty_function(names: &AstNames<'_>, f: DeclId, virtual_static: bool) -> String {
    let ast = names.ast;
    let sink = names.spans;
    let role = |r: FnNameRole| {
        if let Some(s) = sink {
            s.set_role(r);
        }
    };
    let stand_in = AstNames { spans: sink, plain_ast: true, ..AstNames::new(ast) };
    let tnames: &dyn TypeNames = if sink.is_some() { &stand_in } else { ast };
    let n = ast.decls.get(f);
    let mut out = String::new();
    let method = n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord)
        && matches!(
            n.kind(),
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        );
    if virtual_static
        && method
        && let Some(fp) = ast.decls.function(f)
    {
        if fp.spec.is_virtual() {
            out.push_str("virtual ");
        }
        if fp.spec.storage() == FnStorage::Static {
            out.push_str("static ");
        }
    }
    let pattern = ast.function_pattern(f);
    let fp = sugared_proto(ast, pattern);
    let mut proto = String::new();
    role(FnNameRole::Scope);
    names.write_scope_prefix(f, true, &mut proto);
    role(FnNameRole::Name);
    match n.kind() {
        // A constructor or destructor name prints the class name
        // without arguments (clang `DeclarationName::print`).
        DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
            if n.kind() == DeclKind::CXXDestructor {
                proto.push('~');
            }
            if let Some(c) = n.ctx() {
                let plain = names.unrecorded();
                names.span(Some(f), CLASS_NAME, &mut proto, |o| plain.write_name(c, o));
            }
        }
        _ => names.write_name(f, &mut proto),
    }
    role(FnNameRole::Param);
    write_params(ast, tnames, pattern, fp, method, &mut proto);
    let suffix = template_suffix(names, f);
    role(FnNameRole::Return);
    if !suffix.is_empty() {
        proto.push_str(" [");
        proto.push_str(&suffix);
        proto.push(']');
    }
    let lambda = n
        .ctx()
        .and_then(|c| ast.decls.record(c))
        .is_some_and(|r| r.defn.contains(DefinitionBits::LAMBDA));
    if lambda {
        out.push_str("auto ");
        out.push_str(&proto);
        return out;
    }
    if let Some(p) = fp
        && let TypeView::Decltype { underlying, .. } = ast.types.view(p.ret().ty())
    {
        write_declarator(&ast.types, tnames, underlying, PrintOptions::SUGARED, &proto, &mut out);
        return out;
    }
    if matches!(n.kind(), DeclKind::CXXConstructor | DeclKind::CXXDestructor) {
        out.push_str(&proto);
        return out;
    }
    match fp {
        Some(p) => {
            write_declarator(&ast.types, tnames, p.ret(), PrintOptions::SUGARED, &proto, &mut out);
        }
        None => out.push_str(&proto),
    }
    out
}

/// `(params...)` of `pattern` (as written), then a method's cv and ref
/// qualifiers.
fn write_params(
    ast: &Ast,
    names: &dyn TypeNames,
    pattern: DeclId,
    fp: Option<&FuncProto>,
    method: bool,
    proto: &mut String,
) {
    proto.push('(');
    let params: Vec<QualType> = match ast.decls.function(pattern) {
        Some(x) if x.params != crate::ListId::EMPTY => {
            ast.lists.iter::<DeclId>(x.params).filter_map(|p| ast.decls.get(p).ty()).collect()
        }
        _ => fp.map(|p| ast.types.params(p).to_vec()).unwrap_or_default(),
    };
    for (i, &q) in params.iter().enumerate() {
        if i > 0 {
            proto.push_str(", ");
        }
        write_type(&ast.types, names, q, PrintOptions::SUGARED, proto);
    }
    if fp.is_some_and(FuncProto::variadic) {
        if !params.is_empty() {
            proto.push_str(", ");
        }
        proto.push_str("...");
    }
    proto.push(')');
    if method && let Some(p) = fp {
        let info = p.info();
        if info.this_quals().is_const() {
            proto.push_str(" const");
        }
        if info.this_quals().is_volatile() {
            proto.push_str(" volatile");
        }
        match info.ref_qual() {
            RefQual::LValue => proto.push_str(" &"),
            RefQual::RValue => proto.push_str(" &&"),
            RefQual::None => {}
        }
    }
}

/// The `P = A, ...` list: the arguments of the enclosing implicit class
/// template specializations, outermost first, then those of `f` when it
/// is an implicit function template specialization (printed with their
/// types).
fn template_suffix(names: &AstNames<'_>, f: DeclId) -> String {
    let ast = names.ast;
    // The arguments print with the plain policy (no closure or scoped
    // spellings), as clang's `TemplateArgument::print`.
    let plain = AstNames { closure_as_lambda: false, scoped_spec_args: false, ..*names };
    let mut specs = Vec::new();
    let mut cur = ast.decls.get(f).ctx();
    let mut guard = 0;
    while let Some(c) = cur {
        let k = ast.decls.get(c).kind();
        if k == DeclKind::TranslationUnit || guard > 1024 {
            break;
        }
        if k == DeclKind::CXXRecord
            && let Some((_, s)) = ast.spec_info(c)
            && s.kind != SpecKind::ExplicitSpecialization
        {
            specs.push(c);
        }
        guard += 1;
        cur = ast.decls.get(c).ctx();
    }
    let mut out = String::new();
    for &c in specs.iter().rev() {
        write_spec_args(&plain, c, false, &mut out);
    }
    if let Some((_, s)) = ast.spec_info(f)
        && s.kind != SpecKind::ExplicitSpecialization
    {
        write_spec_args(&plain, f, true, &mut out);
    }
    if out.len() >= 2 {
        out.truncate(out.len() - 2);
    }
    out
}

/// Appends `P = A, ` for each named parameter of specialization `d`.
fn write_spec_args(names: &AstNames<'_>, d: DeclId, include_type: bool, out: &mut String) {
    let ast = names.ast;
    let Some((_, s)) = ast.spec_info(d) else { return };
    let Some(info) = ast.template_info(s.template) else { return };
    let params: Vec<DeclId> =
        ast.lists.iter::<DeclId>(ast.templates.param_list(info.params).params).collect();
    let args = ast.types.args(s.args);
    for (i, &p) in params.iter().enumerate() {
        let Some(&a) = args.get(i) else { break };
        let mut pname = String::new();
        names.unrecorded().write_name(p, &mut pname);
        if pname.is_empty() {
            continue;
        }
        if let Some(sink) = names.spans {
            sink.set_role(FnNameRole::Binding);
            write_binding(names, p, &pname, info.pattern, out);
            sink.set_role(FnNameRole::Arg);
        } else {
            out.push_str(&pname);
        }
        out.push_str(" = ");
        let typed = include_type || nttp_has_placeholder_type(ast, p);
        write_arg(names, a, typed, out);
        out.push_str(", ");
    }
}

/// The recorded `P` of `P = A` for template parameter `p` printed
/// `pname`: an entry naming `p`; an abbreviated template's invented
/// parameter (`x:auto`) names, over `x`, the function parameter whose
/// name it borrows (`INVENTED`, spec/ast.md §24.2 R8).
fn write_binding(
    names: &AstNames<'_>,
    p: DeclId,
    pname: &str,
    pattern: Option<DeclId>,
    out: &mut String,
) {
    let ast = names.ast;
    if let Some((x, rest)) = pname.split_once(':')
        && let Some(parm) = invented_function_parm(ast, p, pattern, x)
    {
        names.span(Some(parm), crate::fn_names::INVENTED, out, |o| o.push_str(x));
        out.push(':');
        out.push_str(rest);
        return;
    }
    names.span(Some(p), 0, out, |o| o.push_str(pname));
}

/// The function parameter named `x` of the function whose invented
/// template parameter `p` is (`p` is implicit).
fn invented_function_parm(
    ast: &Ast,
    p: DeclId,
    pattern: Option<DeclId>,
    x: &str,
) -> Option<DeclId> {
    use crate::decl::DeclFlags;
    let n = ast.decls.get(p);
    if n.kind() != DeclKind::TemplateTypeParm || !n.flags().contains(DeclFlags::IMPLICIT) {
        return None;
    }
    // The parameter is made before the function is (its context is the
    // enclosing scope's), so the pattern's parameters are searched.
    let fp = ast.decls.function(pattern?)?;
    ast.lists
        .iter::<DeclId>(fp.params)
        .find(|&q| ast.decls.get(q).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == x))
}

/// Whether template parameter `p` is a non-type parameter with a
/// placeholder type (clang `shouldIncludeTypeForArgument`).
fn nttp_has_placeholder_type(ast: &Ast, p: DeclId) -> bool {
    if ast.decls.get(p).kind() != DeclKind::NonTypeTemplateParm {
        return false;
    }
    let Some(q) = ast.decls.get(p).ty() else { return false };
    let q = ast.types.desugar_top(q);
    matches!(
        ast.types.view(q.ty()),
        TypeView::Auto { .. } | TypeView::DeducedTemplateSpecialization { .. }
    )
}

/// clang `TemplateArgument::print(Policy, Out, IncludeType)`.
fn write_arg(names: &AstNames<'_>, a: TemplateArg, include_type: bool, out: &mut String) {
    let ast = names.ast;
    match a.view() {
        ArgView::Value { value, .. } if include_type => {
            let opts = ValueOptions {
                include_type: true,
                ..ValueOptions::with_style(PrintStyle::TemplateArg)
            };
            ast.consts.print(value, &ast.strings, names, &opts, out);
        }
        // The specialization's arguments are printed canonical, as
        // clang stores them: full scopes, `(anonymous namespace)::`
        // included, and no typedef sugar.
        ArgView::Type(q) => write_type(&ast.types, names, q, PrintOptions::CANONICAL, out),
        ArgView::Pack(l) => {
            out.push('<');
            for (i, &x) in ast.types.args(l).iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_arg(names, x, include_type, out);
            }
            out.push('>');
        }
        _ => write_template_arg(&ast.types, names, a, out),
    }
}
