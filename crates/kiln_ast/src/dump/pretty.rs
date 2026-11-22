//! clang `PredefinedExpr::ComputeName(PrettyFunction, f)`: the
//! `__PRETTY_FUNCTION__` spelling of a function, shared by Sema (the
//! predefined identifiers) and the `-fdump-vtable-layouts` printer
//! (`PrettyFunctionNoVirtual`).

use crate::Ast;
use crate::consts::{PrintOptions as ValueOptions, PrintStyle};
use crate::decl::{DeclKind, DefinitionBits, FnStorage};
use crate::ids::{DeclId, QualType};
use crate::templates::SpecKind;
use crate::types::{
    ArgView, FuncProto, PrintOptions, RefQual, TemplateArg, TypeView, write_declarator,
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
pub fn pretty_function(names: &AstNames<'_>, f: DeclId, virtual_static: bool) -> String {
    let ast = names.ast;
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
    names.write_scope_prefix(f, true, &mut proto);
    match n.kind() {
        // A constructor or destructor name prints the class name
        // without arguments (clang `DeclarationName::print`).
        DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
            if n.kind() == DeclKind::CXXDestructor {
                proto.push('~');
            }
            if let Some(c) = n.ctx() {
                names.write_name(c, &mut proto);
            }
        }
        _ => names.write_name(f, &mut proto),
    }
    write_params(ast, pattern, fp, method, &mut proto);
    let suffix = template_suffix(names, f);
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
        write_declarator(&ast.types, ast, underlying, PrintOptions::SUGARED, &proto, &mut out);
        return out;
    }
    if matches!(n.kind(), DeclKind::CXXConstructor | DeclKind::CXXDestructor) {
        out.push_str(&proto);
        return out;
    }
    match fp {
        Some(p) => {
            write_declarator(&ast.types, ast, p.ret(), PrintOptions::SUGARED, &proto, &mut out);
        }
        None => out.push_str(&proto),
    }
    out
}

/// `(params...)` of `pattern` (as written), then a method's cv and ref
/// qualifiers.
fn write_params(
    ast: &Ast,
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
        write_type(&ast.types, ast, q, PrintOptions::SUGARED, proto);
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
        names.write_name(p, &mut pname);
        if pname.is_empty() {
            continue;
        }
        out.push_str(&pname);
        out.push_str(" = ");
        let typed = include_type || nttp_has_placeholder_type(ast, p);
        write_arg(names, a, typed, out);
        out.push_str(", ");
    }
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
