//! The type printer (spec/types.md §12): a port of clang 19's
//! `TypePrinter` (C++ `PrintingPolicy`), so that diagnostics and AST dumps
//! compare textually with clang's.
//!
//! The algorithm is clang's: a type prints in two halves around the
//! declarator text (the *placeholder*). [`Printer::before`] prints the
//! specifiers and the left part of the declarator (`int (*`), the
//! placeholder follows, then [`Printer::after`] prints the right part
//! (`)[3]`). Qualifiers print before "leaf" kinds (`const int`) and after
//! declarator kinds (`int *const`).
//!
//! Declaration names, expressions and constant values are not owned by
//! the type layer; the caller supplies them through [`TypeNames`]
//! (normally implemented by `Ast`).
//!
//! kiln does not wrap every written type name in `Elaborated` sugar as
//! clang 16+ does (spec/types.md §15 R4). The scope rule below gives the
//! same strings: a bare `Record`/`Enum`/`Typedef` prints unscoped (as
//! written) unless [`PrintOptions::full_scope`] is set; the class of a
//! member pointer and the top-level record of a desugared type print
//! scoped, as clang prints a bare `RecordType`.

use std::fmt::Display;

use super::args::{ArgView, TemplateArg};
use super::context::{ExprOperand, TypeContext, TypeOfOperand, TypeView};
use super::func::ExcSpec;
use super::kinds::{
    BuiltinType, CallConv, Quals, RefQual, TagKeyword, TypeAttrKind, TypeKind, VectorKind,
};
use super::names::TemplateName;
use super::nns::Nns;
use crate::ids::{
    ArgListId, ConstValueId, DeclId, ExprId, NnsId, ProfileId, QualType, RawId, TemplateNameId,
    TypeId,
};
use kiln_source::symbol::Symbol;

/// What the printer needs from the rest of the AST: names of
/// declarations, and the expression and constant-value printers.
pub trait TypeNames {
    /// Writes the name of `decl` (a tag, typedef, using-shadow, template,
    /// template parameter, namespace or concept).
    ///
    /// With `scoped`, the name is preceded by its enclosing namespaces and
    /// classes as clang's `AppendScope` does (`ns::S`, `(anonymous
    /// namespace)::S`, `Outer<int>::Inner`); inline namespaces and
    /// transparent contexts are skipped. Unnamed tags print
    /// `(unnamed struct at F:L:C)`, `(anonymous union at F:L:C)` or
    /// `(lambda at F:L:C)`; a tag named by a typedef for linkage purposes
    /// prints the typedef's name. Unnamed template parameters write
    /// nothing (the printer then uses `type-parameter-D-I`).
    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String);

    /// The template arguments to print after the name of class template
    /// specialization `decl` (`None` for other records). Arguments that
    /// were defaulted should already be omitted, as clang's
    /// `SuppressDefaultTemplateArgs` does.
    fn specialization_args(&self, decl: DeclId) -> Option<ArgListId> {
        let _ = decl;
        None
    }

    /// All the converted template arguments of class template
    /// specialization `decl`, trailing defaults included (`None` for
    /// other records): clang's `getTemplateArgs()`, which a failed
    /// `static_assert` requirement prints ([`write_nns_resolved`]).
    fn converted_args(&self, decl: DeclId) -> Option<ArgListId> {
        self.specialization_args(decl)
    }

    /// Writes the enclosing namespaces and classes of `decl` as clang 22
    /// prints a failed `static_assert` requirement's qualifier (each
    /// class template specialization with all its converted arguments,
    /// canonical: `n::tr<n::A, int>::`, `(anonymous namespace)::q<int>::`).
    /// Inline namespaces are skipped. Returns whether it is implemented.
    fn write_converted_scope(&self, decl: DeclId, out: &mut String) -> bool {
        let _ = (decl, out);
        false
    }

    /// Whether typedef `decl` is implicit (`__builtin_va_list`): its
    /// underlying type was made by the implementation, not written, so
    /// clang 22 prints a tag there with its scope
    /// ([`PrintOptions::written_tags`]).
    fn implicit_typedef(&self, decl: DeclId) -> bool {
        let _ = decl;
        false
    }

    /// The type a class template specialization `decl` prints as, from
    /// its template's `preferred_name` attribute (clang
    /// `PreferredNameAttr`, `printRecordBefore`): the typedef type that
    /// names `decl`. `None` for other records.
    fn preferred_name(&self, decl: DeclId) -> Option<QualType> {
        let _ = decl;
        None
    }

    /// The keyword of class `decl` (`struct`, `class` or `union`), for
    /// [`PrintOptions::c_policy`].
    fn tag_keyword(&self, decl: DeclId) -> &'static str {
        let _ = decl;
        "struct"
    }

    /// For the invented type parameter of an abbreviated function
    /// template ([dcl.fct]/22), which prints as `auto` or `C<A...> auto`:
    /// `Some(constraint)` with the type-constraint's concept and its whole
    /// argument list (the first argument is the parameter itself).
    /// `None` for other declarations.
    fn invented_parm(&self, decl: DeclId) -> Option<Option<(DeclId, ArgListId)>> {
        let _ = decl;
        None
    }

    /// Writes expression `expr` as clang's `Expr::printPretty`.
    fn write_expr(&self, expr: ExprId, out: &mut String);

    /// Writes constant value `value` of type `ty` as a template argument
    /// (spec/ast.md §9.3).
    fn write_value(&self, ty: QualType, value: ConstValueId, out: &mut String);

    /// [`Self::write_value`] with its type shown (a literal suffix or a
    /// cast; clang `TemplateArgument::print` with `IncludeType`).
    fn write_value_typed(&self, ty: QualType, value: ConstValueId, out: &mut String) {
        self.write_value(ty, value, out);
    }

    /// Whether argument `index` of template `template` (a template or one
    /// of its specializations) prints with its type: the parameter is a
    /// non-type parameter with a deduced type (clang
    /// `TemplateParameterList::shouldIncludeTypeForArgument`).
    fn arg_includes_type(&self, template: DeclId, index: usize) -> bool {
        let _ = (template, index);
        false
    }

    /// Writes the spelling of identifier `sym`.
    fn write_symbol(&self, sym: Symbol, out: &mut String);

    /// Writes the name of parameter `index` of template `associated`
    /// (used for unexpanded substituted packs).
    fn write_template_param(&self, associated: DeclId, index: u32, out: &mut String) {
        let _ = associated;
        out.push_str("type-parameter-");
        out.push_str(&index.to_string());
    }

    /// Writes the spelling of overloaded operator `op` (the `kiln_lex`
    /// `TokenKind` discriminant), e.g. `+` or `()`.
    fn write_operator(&self, op: u8, out: &mut String) {
        out.push_str(&op.to_string());
    }

    /// True if `ns` is an anonymous namespace (it prints nothing inside a
    /// nested-name-specifier, as in clang).
    fn is_anonymous_namespace(&self, ns: DeclId) -> bool {
        let _ = ns;
        false
    }

    /// The clang release whose diagnostic type printing to follow
    /// (`-fclang-compat=<major>`, spec/sema.md §1.1). At 22 a canonical
    /// tag type prints fully scoped, inside template arguments too, and
    /// the `aka` keeps a written qualifier ([`aka`]). The AST dump keys
    /// its own clang 22 rules on [`PrintOptions::written_tags`].
    fn clang_compat(&self) -> u32 {
        19
    }
}

/// A [`TypeNames`] without an AST: declarations print as `decl#N`,
/// expressions as `expr#N`, values as `value#N` and symbols as `sym#N`.
/// For debugging and for tests of the type layer alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct RawNames;

impl TypeNames for RawNames {
    fn write_decl_name(&self, decl: DeclId, _scoped: bool, out: &mut String) {
        out.push_str("decl#");
        out.push_str(&decl.raw().to_string());
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        out.push_str("expr#");
        out.push_str(&expr.raw().to_string());
    }

    fn write_value(&self, _ty: QualType, value: ConstValueId, out: &mut String) {
        out.push_str("value#");
        out.push_str(&value.raw().to_string());
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str("sym#");
        out.push_str(&sym.index().to_string());
    }
}

/// Printer options (spec/types.md §12).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent printing policy switches")]
pub struct PrintOptions {
    /// Print `canonical(q)` instead of `q` (clang's `PrintCanonicalTypes`).
    pub canonical: bool,
    /// Remove the top-level sugar first ([`TypeContext::desugar_top`]),
    /// as the second string of clang's AST dump (`'Arr':'int[3]'`).
    pub desugar: bool,
    /// Qualify every declaration name with its scope (`ns::S`).
    pub full_scope: bool,
    /// Omit written elaboration keywords (`struct S` prints `S`).
    pub suppress_tag: bool,
    /// clang's default (C) `PrintingPolicy`, as `QualType::getAsString()`
    /// with no policy uses: `_Bool`, `nullptr_t`, and tag types with
    /// their keyword (`struct S`, `enum E`).
    pub c_policy: bool,
    /// Print `ParenType` sugar as its inner type (no grouping
    /// parentheses): clang's `desugarForDiagnostic` looks through it
    /// without counting it for an `aka` (WP-191).
    pub no_parens: bool,
    /// clang's `SplitTemplateClosers` (the C policy, no C++11): a space
    /// between two `>` closers (`P<P<int> >`), as
    /// `-fdump-record-layouts` prints.
    pub split_closers: bool,
    /// With [`Self::desugar`]: clang 22's rule that a tag type as written
    /// (`S`, `struct q::S`) is not sugar ([`desugar_top_written`]).
    pub written_tags: bool,
}

impl PrintOptions {
    /// The type as written (diagnostics' first string).
    pub const SUGARED: PrintOptions = PrintOptions {
        canonical: false,
        desugar: false,
        full_scope: false,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        written_tags: false,
    };
    /// The canonical type with every name scoped (the `aka` string).
    pub const CANONICAL: PrintOptions = PrintOptions {
        canonical: true,
        desugar: false,
        full_scope: true,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        written_tags: false,
    };
    /// The top-level-desugared type (the AST dump's second string).
    pub const DESUGARED: PrintOptions = PrintOptions {
        canonical: false,
        desugar: true,
        full_scope: false,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        written_tags: false,
    };
    /// The canonical type as clang's policy-less `QualType::getAsString()`
    /// prints it (built-in candidate notes).
    pub const C_DEFAULT: PrintOptions = PrintOptions {
        canonical: true,
        desugar: false,
        full_scope: true,
        suppress_tag: false,
        c_policy: true,
        split_closers: false,
        no_parens: false,
        written_tags: false,
    };
}

/// Renders `q` as clang 19's `QualType::getAsString` (spec/types.md §12).
pub fn print_type(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    q: QualType,
    opts: PrintOptions,
) -> String {
    let mut out = String::new();
    write_type(ctx, names, q, opts, &mut out);
    out
}

/// Like [`print_type`], appending to `out`.
pub fn write_type(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    q: QualType,
    opts: PrintOptions,
    out: &mut String,
) {
    write_declarator(ctx, names, q, opts, "", out);
}

/// Like [`write_type`], with `placeholder` (a declarator name such as
/// `B::f()`) printed inside the type as clang's
/// `QualType::getAsStringInternal(placeholder, policy)` does: `A *B::f()`,
/// `int B::g()`.
pub fn write_declarator(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    q: QualType,
    opts: PrintOptions,
    placeholder: &str,
    out: &mut String,
) {
    let mut q = q;
    if opts.canonical {
        q = ctx.canonical(q);
    }
    let mut scope = opts.full_scope;
    if opts.desugar && opts.written_tags {
        let (d, canonical) = desugar_top_written(ctx, names, q);
        q = d;
        if canonical {
            scope = true;
        }
    } else if opts.desugar {
        q = desugar_top_for_print(ctx, q);
        let (t, _) = split(ctx, q);
        // clang prints a bare RecordType (what desugaring an
        // ElaboratedType leaves) with its scope; likewise an
        // `UnresolvedUsingType` (WP-179).
        if matches!(ctx.kind(t), TypeKind::Record | TypeKind::Enum | TypeKind::UnresolvedUsing) {
            scope = true;
        }
    }
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope,
        suppress_tag: opts.suppress_tag,
        c_policy: opts.c_policy,
        split_closers: opts.split_closers,
        no_parens: opts.no_parens,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: false,
        written_tags: opts.written_tags,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.print(q, placeholder);
    *out = p.out;
}

/// Renders nested-name-specifier `n` as written (clang's
/// `NestedNameSpecifier::print`: `N::M::`, `::`), appending to `out`.
pub fn write_nns(ctx: &TypeContext, names: &dyn TypeNames, n: NnsId, out: &mut String) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: false,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: false,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.nns(n);
    *out = p.out;
}

/// [`write_nns`] as clang's `FailedBooleanConditionPrinterHelper`
/// prints a failed `static_assert` requirement's qualifier
/// (`NestedNameSpecifier::print` with `PrintCanonicalTypes` and
/// `ResolveTemplateArguments`): template arguments print canonical, and
/// a last type component naming a class template specialization prints
/// as its name with its converted arguments (`is_conv<int, bool>::`).
///
/// With `scoped` (clang 22), a type component prints as its canonical
/// type with every name scoped instead (`n::tr<n::A, int>::In::`).
pub fn write_nns_resolved(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    n: NnsId,
    scoped: bool,
    out: &mut String,
) {
    // clang 22 prints the qualifier's record fully scoped, every
    // enclosing specialization with all its converted arguments.
    if scoped
        && let Nns::Type { ty, .. } = ctx.nns_view(n)
        && let TypeView::Record(d) = ctx.view(ctx.canonical(ty).ty())
    {
        let mut s = String::new();
        if names.write_converted_scope(d, &mut s) {
            names.write_decl_name(d, false, &mut s);
            if let Some(args) = names.converted_args(d) {
                write_arg_list_canonical(ctx, names, args, &mut s);
            }
            out.push_str(&s);
            out.push_str("::");
            return;
        }
    }
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: true,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: true,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    let view = ctx.nns_view(n);
    let resolved = match view {
        Nns::Type { ty, .. } => match ctx.view(ctx.canonical(ty).ty()) {
            TypeView::Record(d) => names.converted_args(d).map(|l| (d, l)),
            _ => None,
        },
        _ => None,
    };
    match (resolved, view) {
        (Some((d, args)), Nns::Type { prefix, .. }) => {
            if let Some(pre) = prefix {
                p.nns(pre);
            }
            names.write_decl_name(d, false, &mut p.out);
            p.arg_list_prefix(ctx.canonical_args(args), false, usize::MAX, Some(d), false);
            p.push("::");
        }
        _ => p.nns(n),
    }
    *out = p.out;
}

/// [`write_arg_list_of`] with every argument canonical (clang printing
/// explicit template arguments with `PrintCanonicalTypes`): `<int>` for
/// `<X>` with `using X = int`.
pub fn write_arg_list_canonical(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    l: ArgListId,
    out: &mut String,
) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: true,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: true,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.arg_list(l, false);
    *out = p.out;
}

/// Renders template-argument list `l` as written (clang's
/// `printTemplateArgumentList`: `<int, N>`), appending to `out`.
pub fn write_arg_list(ctx: &TypeContext, names: &dyn TypeNames, l: ArgListId, out: &mut String) {
    arg_list_with_scope(ctx, names, l, false, None, out);
}

/// [`write_arg_list`] with every type argument's scope printed (clang
/// printing canonical arguments, which carry no `ElaboratedType` to
/// suppress it): `<std::alloc<int>>`.
pub fn write_arg_list_scoped(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    l: ArgListId,
    out: &mut String,
) {
    arg_list_with_scope(ctx, names, l, true, None, out);
}

/// [`write_arg_list_scoped`] of the arguments of template (or
/// specialization) `template` ([`write_arg_list_of`]).
pub fn write_arg_list_scoped_of(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    template: DeclId,
    l: ArgListId,
    out: &mut String,
) {
    arg_list_with_scope(ctx, names, l, true, Some(template), out);
}

fn arg_list_with_scope(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    l: ArgListId,
    scope: bool,
    template: Option<DeclId>,
    out: &mut String,
) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: scope,
        canon_args: false,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.arg_list_prefix(l, false, usize::MAX, template, false);
    *out = p.out;
}

/// [`write_arg_list`] of the arguments of template (or specialization)
/// `template`, whose values print with their type where its parameter
/// asks for it ([`TypeNames::arg_includes_type`]).
pub fn write_arg_list_of(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    template: DeclId,
    l: ArgListId,
    out: &mut String,
) {
    arg_list_with_scope(ctx, names, l, false, Some(template), out);
}

/// The arguments `l` of class template specialization `template` as
/// clang prints a specialization's name under the C policy
/// (`QualType::getAsString()` of its canonical type, as
/// `-fdump-record-layouts` does): canonical, scoped, tag types with
/// their keyword, `> >` between closers (`<struct ns::Q, P<int> >`).
pub fn write_arg_list_c(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    template: DeclId,
    l: ArgListId,
    out: &mut String,
) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: true,
        suppress_tag: false,
        c_policy: true,
        split_closers: true,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: true,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.arg_list_prefix(l, false, usize::MAX, Some(template), false);
    *out = p.out;
}

/// [`write_arg_list`] of the first `len` arguments of `l` (`<>` when
/// `len` is 0): a name printed without its trailing default arguments.
pub fn write_arg_list_prefix(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    l: ArgListId,
    len: usize,
    out: &mut String,
) {
    arg_list_prefix_with_scope(ctx, names, l, len, false, out);
}

/// [`write_arg_list_prefix`] with every type argument's scope printed,
/// as [`write_arg_list_scoped`] (`<T *, N::M::Y>`).
pub fn write_arg_list_prefix_scoped(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    l: ArgListId,
    len: usize,
    out: &mut String,
) {
    arg_list_prefix_with_scope(ctx, names, l, len, true, out);
}

fn arg_list_prefix_with_scope(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    l: ArgListId,
    len: usize,
    scope: bool,
    out: &mut String,
) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: scope,
        canon_args: false,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.arg_list_prefix(l, false, len, None, false);
    *out = p.out;
}

/// Renders one template argument as written (no brackets), appending to
/// `out`.
pub fn write_template_arg(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    a: TemplateArg,
    out: &mut String,
) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: false,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: false,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.arg(a.view());
    *out = p.out;
}

/// Writes template name `n` as written (clang's `TemplateName::print`
/// with `Qualified::AsWritten`): `nns::[template] X` keeps its qualifier,
/// a bare name stays bare.
pub fn write_template_name(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    n: TemplateNameId,
    out: &mut String,
) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: false,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: false,
        in_tst_args: false,
        scoped_args: false,
        canon_args: false,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.template_name(n, Qual::AsWritten);
    *out = p.out;
}

/// The diagnostic `aka` string: the canonical type printed with every
/// name scoped, when it differs textually from the type as written
/// (`'S' (aka 'ns::S')`).
///
/// clang's `desugarForDiagnostic` keeps a function type's exception
/// specification as written, so a difference only there (`noexcept(false)`
/// against its canonical nothing) gives no `aka`.
pub fn aka(ctx: &TypeContext, names: &dyn TypeNames, q: QualType) -> Option<String> {
    // Only sugar clang counts gives an `aka`: a type whose canonical form
    // differs from its written form only by scope (`'plain'` for
    // `view<int>::plain`, written in the class) has none (WP-287).
    if !should_aka(ctx, q, 0) {
        return None;
    }
    let written = print_type(ctx, names, q, PrintOptions::SUGARED);
    let canon = print_diag_desugared(ctx, names, q);
    // clang's `Desugar` keeps template type parameters by name (`'pr<T, I>'
    // (aka 'pr<T, int>')`); the canonical printer cannot, so a dependent
    // type whose canonical form names `type-parameter-D-I` gets no `aka`
    // rather than a wrong one.
    if canon.contains("type-parameter-") && !written.contains("type-parameter-") {
        return None;
    }
    // clang's `desugarForDiagnostic` steps through an `ElaboratedType`
    // without asking for an `aka`: `'struct S'` is not `'struct S' (aka
    // 'S')`.
    if without_tag_keywords(&written) == canon {
        return None;
    }
    // clang's `desugarForDiagnostic` looks through `ParenType` without
    // counting it for an `aka`: `'int (*)'`, not `'int (*)' (aka 'int *')`.
    if canon != written && written.contains('(') {
        let np = PrintOptions { no_parens: true, ..PrintOptions::SUGARED };
        if without_tag_keywords(&print_type(ctx, names, q, np)) == canon {
            return None;
        }
    }
    (canon != written && without_exc_specs(&canon) != without_exc_specs(&written)).then_some(canon)
}

/// `q` as clang prints `desugarForDiagnostic(q)`: every name scoped, sugar
/// looked through except a class template specialization's, whose name
/// prints unqualified (`'J<ns::S>'` for `O::J<SA>`, `'basic_string<char>'`
/// for `std::string`; WP-291).
fn print_diag_desugared(ctx: &TypeContext, names: &dyn TypeNames, q: QualType) -> String {
    let mut p = Printer {
        ctx,
        names,
        out: String::new(),
        empty_ph: true,
        scope: true,
        suppress_tag: false,
        c_policy: false,
        split_closers: false,
        no_parens: false,
        inside_cc_attr: false,
        diag_desugar: true,
        in_tst_args: false,
        scoped_args: false,
        canon_args: false,
        written_tags: false,
        c22: names.clang_compat() >= 22,
        depth: 0,
    };
    p.print(q, "");
    p.out
}

/// A type argument whose canonical type is a vector, as clang prints it
/// in a diagnostic (`ConvertTypeToDiagnosticString`): `'T' (aka 'U')`
/// when desugaring looks through sugar that counts, else
/// `'T' (vector of N 'E' values)`; `None` for a non-vector type. Shared
/// by Sema's `ty_arg` and codegen's `-Wpsabi`.
pub fn vector_diag_arg(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    q: QualType,
    keep_typename: bool,
) -> Option<String> {
    let TypeView::Vector { elem, count, .. } = ctx.canonical_view(q) else {
        return None;
    };
    let s = print_type(ctx, names, q, PrintOptions::SUGARED);
    // clang's `Desugar` stops before a vector type ("people want their
    // vec4"), and prints the vector details only without an `aka`.
    if let Some(d) = vector_aka(ctx, q) {
        let a = print_type(ctx, names, d, PrintOptions::SUGARED);
        // A substituted `typename t1<I>::v` prints as clang 19's rebuilt
        // qualified name, without the keyword (`keep_typename`: clang 22
        // keeps it).
        let a = match a.strip_prefix("typename ") {
            Some(rest) if !keep_typename => rest.to_owned(),
            _ => a.clone(),
        };
        if a != s {
            return Some(format!("'{s}' (aka '{a}')"));
        }
    }
    let top = ctx.desugar_top(q);
    let elem = match ctx.view(top.ty()) {
        TypeView::Vector { elem, .. } => elem,
        _ => elem,
    };
    let e = print_type(ctx, names, elem, PrintOptions::SUGARED);
    let v = if count == 1 { "value" } else { "values" };
    Some(format!("'{s}' (vector of {count} '{e}' {v})"))
}

/// clang's `Desugar` for a type whose canonical type is a vector: the
/// type desugared step by step, stopping at the sugar whose underlying
/// type is the vector itself, or at the qualified name of the vector's
/// typedef (`'B' (aka 'X::v')`); `None` when no step counts for an
/// `aka` (template parameter substitution, `using` and elaborated
/// names, attributes and deduced types do not).
fn vector_aka(t: &TypeContext, q: QualType) -> Option<QualType> {
    let mut cur = q;
    let mut aka = false;
    for _ in 0..MAX_DEPTH {
        let next = t.desugar_once(cur);
        if next == cur {
            break;
        }
        match t.view(cur.ty()) {
            // clang substitutes canonical template arguments.
            TypeView::SubstTemplateTypeParm { .. } => {
                cur = t.canonical(cur);
                continue;
            }
            TypeView::Elaborated { .. }
                if matches!(t.view(t.desugar_once(next).ty()), TypeView::Vector { .. }) =>
            {
                break;
            }
            TypeView::Elaborated { .. }
            | TypeView::Using { .. }
            | TypeView::Attributed { .. }
            | TypeView::Auto { .. } => {}
            _ => {
                if matches!(t.view(next.ty()), TypeView::Vector { .. }) {
                    break;
                }
                aka = true;
            }
        }
        cur = next;
    }
    aka.then_some(cur)
}

/// clang's `desugarForDiagnostic` `ShouldAKA`: whether desugaring `q`
/// for a diagnostic looks through any sugar that asks for an `aka`.
/// Elaborated, `using`, parenthesized, attributed and deduced (`auto`)
/// types and template parameter substitutions do not; nor does a class
/// template specialization's name, though its type arguments are
/// desugared, as are a function type's return and parameter types, an
/// array's element and a pointer's or reference's pointee. Any other
/// sugar (typedef, alias template, `decltype`, ...) does, except sugar
/// for a vector type.
fn should_aka(ctx: &TypeContext, q: QualType, depth: u32) -> bool {
    if depth >= MAX_DEPTH {
        return false;
    }
    let deeper = |q: QualType| should_aka(ctx, q, depth + 1);
    let mut cur = q;
    for _ in 0..MAX_DEPTH {
        match ctx.view(cur.ty()) {
            TypeView::ExtQual { base, .. } => {
                cur = base;
                continue;
            }
            // clang substitutes canonical template arguments, so nothing
            // under a non-dependent substitution is sugar: an explicit or
            // deduced argument written as a typedef (`mk<s::pl>()`'s
            // `W<T>`) prints `'W<plain>'` with no `aka` (WP-366).
            TypeView::SubstTemplateTypeParm { .. } if !ctx.is_dependent(cur) => return false,
            TypeView::Elaborated { .. }
            | TypeView::Using { .. }
            | TypeView::Paren(_)
            | TypeView::SubstTemplateTypeParm { .. }
            | TypeView::Attributed { .. }
            | TypeView::Auto { deduced: Some(_), .. } => {
                cur = ctx.desugar_once(cur);
                continue;
            }
            TypeView::Function(p) => {
                if deeper(p.ret()) || ctx.params(&p).iter().any(|&t| deeper(t)) {
                    return true;
                }
                break;
            }
            TypeView::TemplateSpecialization { args, alias: false, .. } => {
                return ctx.args(args).iter().any(|a| match a.view() {
                    ArgView::Type(t) => deeper(t),
                    _ => false,
                });
            }
            TypeView::ConstantArray(a) => return deeper(a.elem),
            TypeView::IncompleteArray(e)
            | TypeView::VariableArray { elem: e, .. }
            | TypeView::DependentSizedArray { elem: e, .. } => return deeper(e),
            TypeView::QualifiedArray { base, .. } => return deeper(QualType::unqual(base)),
            _ => {}
        }
        let next = ctx.desugar_once(cur);
        if next == cur
            || matches!(
                ctx.view(next.ty()),
                TypeView::Vector { .. } | TypeView::DependentVector { .. }
            )
        {
            break;
        }
        return true;
    }
    match ctx.view(cur.ty()) {
        TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => deeper(p),
        _ => false,
    }
}

/// `s` with the elaborated-type keywords `struct `, `class `, `union `
/// and `enum ` removed where they begin a word.
fn without_tag_keywords(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while !rest.is_empty() {
        let at_word = out.chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if at_word {
            for k in ["struct ", "class ", "union ", "enum "] {
                if let Some(r) = rest.strip_prefix(k) {
                    rest = r;
                    continue 'outer;
                }
            }
        }
        let Some(c) = rest.chars().next() else { break };
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// `s` with every printed exception specification (` noexcept`,
/// ` noexcept(...)`, ` throw(...)`) removed.
fn without_exc_specs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let next = [" noexcept", " throw("]
            .iter()
            .filter_map(|k| rest.find(k).map(|i| (i, k.len())))
            .min();
        let Some((i, n)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..i]);
        rest = &rest[i + n..];
        // The balanced parenthesized operand, if any (`throw(` opened one).
        let mut depth = usize::from(n == " throw(".len());
        if depth == 0 && rest.starts_with('(') {
            depth = 1;
            rest = &rest[1..];
        }
        while depth > 0 {
            let Some(c) = rest.chars().next() else { break };
            rest = &rest[c.len_utf8()..];
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
    }
}

/// The two strings of clang's AST dump (`'Arr':'int[3]'`): the type as
/// written and, when top-level desugaring changes the type or its text,
/// the desugared type.
///
/// clang prints the second string whenever the type is sugar, including
/// the `ElaboratedType` it wraps around every written name (`'S':'S'`);
/// kiln creates no such node (spec/types.md §15 R4), so the second string
/// is omitted when it would repeat the first.
pub fn dump_strings(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    q: QualType,
) -> (String, Option<String>) {
    let written = print_type(ctx, names, q, PrintOptions::SUGARED);
    let desugared = print_type(ctx, names, q, PrintOptions::DESUGARED);
    let differs = desugar_top_for_print(ctx, q) != q || desugared != written;
    (written, differs.then_some(desugared))
}

/// [`TypeContext::desugar_top`], except that an undeduced `auto` whose
/// initializer is type-dependent (kiln: deduced to `<dependent type>`)
/// is kept: clang's `AutoType` is canonical there and prints as `auto`.
fn desugar_top_for_print(ctx: &TypeContext, mut q: QualType) -> QualType {
    loop {
        if matches!(ctx.view(q.ty()), TypeView::Auto { deduced: Some(d), .. } if d == QualType::DEPENDENT)
        {
            return q;
        }
        let n = ctx.desugar_once(q);
        if n == q {
            return q;
        }
        q = n;
    }
}

/// clang 22's top-level desugaring (`-fclang-compat=22`): since clang 22
/// a tag type carries its written keyword and qualifier and is not sugar,
/// so desugaring stops at a tag as written (kiln: an `Elaborated` node
/// naming a record or enumeration, or a bare record or enumeration
/// reached through written sugar such as a typedef), which then prints
/// as written. The second value says the tag was reached through
/// canonicalizing sugar (a substituted template parameter, a deduced
/// type, `decltype`, a using-declaration's `UsingType`, a template
/// specialization's name), whose replacement clang stores canonical and
/// prints with its scope (`'T':'q::S'`, `'S<int>':'q::S<int>'`).
fn desugar_top_written(
    ctx: &TypeContext,
    names: &dyn TypeNames,
    mut q: QualType,
) -> (QualType, bool) {
    let mut canonical = false;
    loop {
        if matches!(ctx.view(q.ty()), TypeView::Auto { deduced: Some(d), .. } if d == QualType::DEPENDENT)
        {
            return (q, canonical);
        }
        let view = ctx.view(split(ctx, q).0);
        if let TypeView::Elaborated { mut named, .. } = view {
            // `typename S` wraps the written `S` (itself `Elaborated`).
            while let TypeView::Elaborated { named: inner, .. } = ctx.view(split(ctx, named).0) {
                named = inner;
            }
            if matches!(ctx.kind(split(ctx, named).0), TypeKind::Record | TypeKind::Enum) {
                return (q, false);
            }
        }
        let n = ctx.desugar_once(q);
        if n == q {
            return (q, canonical);
        }
        match view {
            TypeView::Typedef { decl, .. } => canonical = names.implicit_typedef(decl),
            TypeView::Elaborated { .. } => canonical = false,
            TypeView::Paren(_) | TypeView::Attributed { .. } | TypeView::QualifiedArray { .. } => {}
            _ => canonical = true,
        }
        q = n;
    }
}

/// Nesting beyond this prints `...` instead of recursing (no stack
/// exhaustion on adversarial types).
const MAX_DEPTH: u32 = 400;

/// How a template name is qualified (clang's `TemplateName::Qualified`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Qual {
    None,
    AsWritten,
    Fully,
}

/// `q` with `QualifiedArray` sugar looked through: the node whose quals
/// are printed and the qualifiers (clang stores `const A` as `A` + const).
fn split(ctx: &TypeContext, q: QualType) -> (TypeId, Quals) {
    let (mut t, mut quals) = (q.ty(), q.quals());
    while let Some(TypeView::QualifiedArray { quals: qa, base }) =
        ctx.view_if(t, TypeKind::QualifiedArray)
    {
        // `base` was created before `t`, so this terminates.
        if base.index() >= t.index() {
            break;
        }
        t = base;
        quals |= qa;
    }
    (t, quals)
}

#[allow(clippy::struct_excessive_bools, reason = "clang's TypePrinter state bits")]
struct Printer<'a> {
    ctx: &'a TypeContext,
    names: &'a dyn TypeNames,
    out: String,
    /// clang's `HasEmptyPlaceHolder`.
    empty_ph: bool,
    /// clang's `!SuppressScope`.
    scope: bool,
    suppress_tag: bool,
    /// [`PrintOptions::c_policy`].
    c_policy: bool,
    /// [`PrintOptions::split_closers`].
    split_closers: bool,
    /// [`PrintOptions::no_parens`].
    no_parens: bool,
    /// clang's `InsideCCAttribute`.
    inside_cc_attr: bool,
    /// Print every node as clang's `desugarForDiagnostic` leaves it (the
    /// `aka` string, [`aka`]): sugar is looked through, except a class
    /// template specialization's name, which prints unqualified with its
    /// arguments desugared.
    diag_desugar: bool,
    /// Printing a written template-id's arguments. clang wraps every
    /// written type name in an `ElaboratedType`, whose policy suppresses
    /// scope for everything inside it, so a substituted class prints
    /// unscoped there (`'Box<Q, P>'`) and scoped elsewhere (`'N::Q *'`).
    in_tst_args: bool,
    /// The printer stands for clang printing canonical arguments
    /// ([`write_arg_list_scoped`]), which carry no `ElaboratedType`, so
    /// [`Printer::in_tst_args`] never applies.
    scoped_args: bool,
    /// Print template arguments canonical (clang's `PrintCanonicalTypes`
    /// in `TemplateArgument::print`), with their scope.
    canon_args: bool,
    /// [`PrintOptions::written_tags`].
    written_tags: bool,
    /// [`TypeNames::clang_compat`] is 22 or later.
    c22: bool,
    depth: u32,
}

impl Printer<'_> {
    fn push(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn num(&mut self, n: impl Display) {
        self.out.push_str(&n.to_string());
    }

    /// Whether a `ParenType` around `inner` prints its parentheses.
    fn paren_shown(&self, inner: QualType) -> bool {
        !self.empty_ph
            && !self.no_parens
            && self.ctx.kind(split(self.ctx, inner).0) != TypeKind::Function
    }

    fn space_before_ph(&mut self) {
        if !self.empty_ph {
            self.out.push(' ');
        }
    }

    /// Prints a `SubstTemplateTypeParm`'s replacement with `f`. clang
    /// substitutes canonical template arguments, and prints the bare
    /// `RecordType` that leaves scoped: `'view<int>::iterator<false>'`
    /// (WP-287). A dependent replacement (a partial substitution) keeps
    /// its parameter names.
    ///
    /// clang 22 keeps a sugared replacement (an alias template's
    /// arguments as written, `'Ptr<I>':'I *'`) and prints it as it is.
    fn subst(&mut self, replacement: QualType, f: fn(&mut Self, QualType)) {
        if self.ctx.is_dependent(replacement)
            || (self.written_tags && self.ctx.canonical(replacement) != replacement)
        {
            f(self, replacement);
        } else {
            let r = self.ctx.canonical(replacement);
            // clang 22 prints a canonical tag fully scoped, inside a
            // written template-id's arguments too (`'Box<N::Q, int>'`).
            let scope = !self.in_tst_args || self.c22;
            self.with_scope(scope, |p| f(p, r));
        }
    }

    /// clang `printRecordBefore`'s `UsePreferredNames`: a specialization
    /// named by its template's `preferred_name` typedef prints as that
    /// typedef (with its scope, as a bare `TypedefType`), canonical
    /// arguments (`PrintCanonicalTypes`) included; not in C.
    fn preferred_name(&mut self, d: DeclId) -> bool {
        if self.c_policy {
            return false;
        }
        let Some(mut t) = self.names.preferred_name(d) else { return false };
        loop {
            match self.ctx.view(t.ty()) {
                TypeView::Elaborated { named, .. } => t = named,
                TypeView::Typedef { decl, .. } => {
                    self.decl(decl, self.scope);
                    return true;
                }
                _ => return false,
            }
        }
    }

    fn with_scope(&mut self, scope: bool, f: impl FnOnce(&mut Self)) {
        let saved = self.scope;
        self.scope = scope;
        f(self);
        self.scope = saved;
    }

    /// clang's `print(T, OS, PlaceHolder)`.
    fn print(&mut self, q: QualType, placeholder: &str) {
        let saved = self.empty_ph;
        self.empty_ph = placeholder.is_empty();
        self.before(q);
        self.out.push_str(placeholder);
        self.after(q);
        self.empty_ph = saved;
    }

    fn decl(&mut self, d: DeclId, scoped: bool) {
        let names = self.names;
        names.write_decl_name(d, scoped, &mut self.out);
    }

    fn expr(&mut self, e: ExprId) {
        let names = self.names;
        names.write_expr(e, &mut self.out);
    }

    fn profile(&mut self, p: ProfileId) {
        match self.ctx.profile_expr(p) {
            Some(e) => self.expr(e),
            None => self.push("<expr>"),
        }
    }

    fn operand(&mut self, o: ExprOperand) {
        match o {
            ExprOperand::Expr(e) => self.expr(e),
            ExprOperand::Profile(p) => self.profile(p),
        }
    }

    fn symbol(&mut self, s: Symbol) {
        let names = self.names;
        names.write_symbol(s, &mut self.out);
    }

    /// True if the node of `q` itself is an array (clang's
    /// `isa<ArrayType>(QualType)`: sugar does not count).
    fn is_array_node(&self, q: QualType) -> bool {
        let (t, _) = split(self.ctx, q);
        self.ctx.kind(t).is_array()
    }

    /// clang's `canPrefixQualifiers`.
    fn can_prefix_quals(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        let mut t = t;
        for _ in 0..MAX_DEPTH {
            match ctx.view(t) {
                TypeView::SubstTemplateTypeParm { replacement, .. } => t = replacement.ty(),
                TypeView::Auto { deduced: Some(d), .. } if d != QualType::DEPENDENT => {
                    t = ctx.canonical(d).ty();
                }
                TypeView::QualifiedArray { base, .. } => t = base,
                TypeView::ConstantArray(a) => t = a.elem.ty(),
                TypeView::IncompleteArray(e)
                | TypeView::VariableArray { elem: e, .. }
                | TypeView::DependentSizedArray { elem: e, .. } => t = e.ty(),
                TypeView::PackIndexing { pattern, .. } => t = pattern.ty(),
                v => {
                    return !matches!(
                        v,
                        TypeView::Pointer(_)
                            | TypeView::LValueRef(_)
                            | TypeView::RValueRef(_)
                            | TypeView::MemberPointer { .. }
                            | TypeView::Vector { .. }
                            | TypeView::DependentVector { .. }
                            | TypeView::Function(_)
                            | TypeView::Paren(_)
                            | TypeView::PackExpansion { .. }
                            | TypeView::SubstTemplateTypeParm { .. }
                            | TypeView::Attributed { .. }
                    );
                }
            }
        }
        true
    }

    /// clang's `printBefore(QualType)`.
    fn before(&mut self, q: QualType) {
        if self.depth >= MAX_DEPTH {
            self.push("...");
            return;
        }
        self.depth += 1;
        let q = if self.diag_desugar { self.diag_strip(q) } else { q };
        let q = if self.canon_args { self.ctx.canonical(q) } else { q };
        let (t, mut quals) = split(self.ctx, q);
        if self.ctx.kind(t) == TypeKind::SubstTemplateTypeParm {
            // cv1 T where T was substituted by cv2 U: print cv1 - cv2 here.
            quals = quals - self.ctx.canonical(QualType::unqual(t)).quals();
        }
        self.before_node(t, quals);
        self.depth -= 1;
    }

    /// clang's `printAfter(QualType)`.
    fn after(&mut self, q: QualType) {
        if self.depth >= MAX_DEPTH {
            return;
        }
        self.depth += 1;
        let q = if self.diag_desugar { self.diag_strip(q) } else { q };
        let q = if self.canon_args { self.ctx.canonical(q) } else { q };
        let (t, _) = split(self.ctx, q);
        self.after_node(t);
        self.depth -= 1;
    }

    fn before_node(&mut self, t: TypeId, quals: Quals) {
        let saved_ph = self.empty_ph;
        let prefix = self.can_prefix_quals(t);
        if prefix && !quals.is_empty() {
            quals.write(&mut self.out);
            self.out.push(' ');
        }
        let after_quals = !prefix && !quals.is_empty();
        if after_quals {
            self.empty_ph = false;
        }
        self.before_kind(t);
        if after_quals {
            quals.write(&mut self.out);
            if !saved_ph {
                self.out.push(' ');
            }
        }
        self.empty_ph = saved_ph;
    }

    #[allow(clippy::too_many_lines, reason = "one arm per type kind, as clang's TypePrinter")]
    fn before_kind(&mut self, t: TypeId) {
        let ctx = self.ctx;
        match ctx.view(t) {
            TypeView::Invalid => {
                self.push("<invalid type>");
                self.space_before_ph();
            }
            TypeView::Builtin(b) => {
                let name = match b {
                    BuiltinType::Bool if self.c_policy => "_Bool",
                    BuiltinType::NullPtr if self.c_policy => "nullptr_t",
                    _ => b.name(),
                };
                self.push(name);
                self.space_before_ph();
            }
            TypeView::BitInt { signed, width } => {
                if !signed {
                    self.push("unsigned ");
                }
                self.push("_BitInt(");
                self.num(width);
                self.push(")");
                self.space_before_ph();
            }
            TypeView::DependentBitInt { signed, width } => {
                if !signed {
                    self.push("unsigned ");
                }
                self.push("_BitInt(");
                self.profile(width);
                self.push(")");
                self.space_before_ph();
            }
            TypeView::Pointer(p) => self.declarator_before(p, "*"),
            TypeView::LValueRef(p) => self.declarator_before(p, "&"),
            TypeView::RValueRef(p) => self.declarator_before(p, "&&"),
            TypeView::MemberPointer { pointee, class } => {
                let saved = self.empty_ph;
                self.empty_ph = false;
                self.before(pointee);
                if self.is_array_node(pointee) {
                    self.push("(");
                }
                // clang 19 prints the class as a bare (scoped) type; clang
                // 22 as its written qualifier (`int P::*`).
                let scope = !self.written_tags || self.scope;
                self.with_scope(scope, |p| p.print(QualType::unqual(class), ""));
                self.push("::*");
                self.empty_ph = saved;
            }
            TypeView::ConstantArray(a) => self.before(a.elem),
            TypeView::IncompleteArray(e)
            | TypeView::VariableArray { elem: e, .. }
            | TypeView::DependentSizedArray { elem: e, .. } => self.before(e),
            TypeView::Function(_) => self.function_before(t),
            TypeView::Record(d) => {
                if self.preferred_name(d) {
                    self.space_before_ph();
                    return;
                }
                if self.c_policy {
                    let names = self.names;
                    self.push(names.tag_keyword(d));
                    self.push(" ");
                }
                self.decl(d, self.scope);
                // clang's `printTo` keeps trailing default arguments
                // under `PrintCanonicalTypes`.
                let args = if self.canon_args {
                    self.names.converted_args(d).map(|l| self.ctx.canonical_args(l))
                } else {
                    self.names.specialization_args(d)
                };
                if let Some(args) = args {
                    if self.c22 {
                        // clang 22: the specialization's (canonical)
                        // arguments print fully scoped, whatever the
                        // record's own name does (`'const J<ns::S> &'`).
                        let saved = self.in_tst_args;
                        self.in_tst_args = false;
                        self.with_scope(true, |p| {
                            p.arg_list_prefix(args, false, usize::MAX, Some(d), false);
                        });
                        self.in_tst_args = saved;
                    } else {
                        self.arg_list_prefix(args, false, usize::MAX, Some(d), false);
                    }
                }
                self.space_before_ph();
            }
            TypeView::Enum(d) => {
                if self.c_policy {
                    self.push("enum ");
                }
                self.decl(d, self.scope);
                self.space_before_ph();
            }
            TypeView::UnresolvedUsing(d) => {
                self.decl(d, self.scope);
                self.space_before_ph();
            }
            // clang's printParenBefore: the parentheses are printed only
            // inside a declarator and not around a function type (which
            // prints its own).
            TypeView::Paren(inner) => {
                self.before(inner);
                if self.paren_shown(inner) {
                    self.push("(");
                }
            }
            TypeView::Complex(e) => {
                self.push("_Complex ");
                self.before(e);
            }
            TypeView::Atomic(v) => {
                self.push("_Atomic(");
                self.print(v, "");
                self.push(")");
                self.space_before_ph();
            }
            TypeView::Vector { elem, count, kind } => {
                match kind {
                    VectorKind::Generic => {
                        self.push("__attribute__((__vector_size__(");
                        self.num(count);
                        self.push(" * sizeof(");
                        self.print(elem, "");
                        self.push(")))) ");
                    }
                    VectorKind::Neon | VectorKind::NeonPoly => {
                        self.push(if kind == VectorKind::Neon {
                            "__attribute__((neon_vector_type("
                        } else {
                            "__attribute__((neon_polyvector_type("
                        });
                        self.num(count);
                        self.push("))) ");
                    }
                    VectorKind::Ext => {}
                }
                self.before(elem);
            }
            TypeView::DependentVector { elem, size, kind } => {
                match kind {
                    VectorKind::Generic => {
                        self.push("__attribute__((__vector_size__(");
                        self.operand(size);
                        self.push(" * sizeof(");
                        self.print(elem, "");
                        self.push(")))) ");
                    }
                    VectorKind::Neon | VectorKind::NeonPoly => {
                        self.push(if kind == VectorKind::Neon {
                            "__attribute__((neon_vector_type("
                        } else {
                            "__attribute__((neon_polyvector_type("
                        });
                        self.operand(size);
                        self.push("))) ");
                    }
                    VectorKind::Ext => {}
                }
                self.before(elem);
            }
            TypeView::ExtQual { base, word } => {
                self.push("__attribute__((address_space(");
                self.num(word);
                self.push("))) ");
                self.before(base);
            }
            TypeView::Typedef { decl, .. } | TypeView::Using { shadow: decl, .. } => {
                self.decl(decl, self.scope);
                self.space_before_ph();
            }
            TypeView::Elaborated { keyword, nns, named } => {
                if keyword != TagKeyword::None && !self.suppress_tag {
                    self.push(keyword.spelling());
                    self.push(" ");
                }
                if let Some(n) = nns {
                    self.nns(n);
                }
                self.with_scope(false, |p| p.before(named));
            }
            TypeView::QualifiedArray { quals, base } => {
                // Unreachable through `split`; print transparently.
                self.before(QualType::new(base, quals.bits()));
            }
            TypeView::TemplateTypeParm(d) => {
                let names = self.names;
                if let Some(c) = names.invented_parm(d) {
                    // clang's `printTemplateTypeParmBefore` for an implicit
                    // parameter: the type-constraint, then `auto`.
                    if let Some((concept, l)) = c {
                        self.decl(concept, false);
                        let ctx = self.ctx;
                        let extra = &ctx.args(l)[1.min(ctx.args(l).len())..];
                        if !extra.is_empty() {
                            self.push("<");
                            for (i, a) in extra.iter().enumerate() {
                                if i > 0 {
                                    self.push(", ");
                                }
                                self.arg(a.view());
                            }
                            self.push(">");
                        }
                        self.push(" ");
                    }
                    self.push("auto");
                } else {
                    let start = self.out.len();
                    self.decl(d, false);
                    if self.out.len() == start
                        && let TypeView::CanonTemplateTypeParm { depth, index, .. } =
                            ctx.canonical_view(QualType::unqual(t))
                    {
                        self.parm_name(depth, index);
                    }
                }
                self.space_before_ph();
            }
            TypeView::CanonTemplateTypeParm { depth, index, .. } => {
                self.parm_name(depth, index);
                self.space_before_ph();
            }
            TypeView::SubstTemplateTypeParm { replacement, .. } => {
                self.subst(replacement, Self::before);
            }
            TypeView::SubstTemplateTypeParmPack { associated, index, .. } => {
                let names = self.names;
                names.write_template_param(associated, index, &mut self.out);
                self.space_before_ph();
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                self.template_id(name, args);
                self.space_before_ph();
            }
            TypeView::InjectedClassName { injected, .. } => {
                match ctx.view(injected.ty()) {
                    TypeView::TemplateSpecialization { name, args, .. } => {
                        self.template_id(name, args);
                    }
                    _ => self.print(injected, ""),
                }
                self.space_before_ph();
            }
            TypeView::DependentName { keyword, nns, name } => {
                self.keyword(keyword);
                self.nns(nns);
                self.symbol(name);
                self.space_before_ph();
            }
            TypeView::DependentTemplateSpecialization { keyword, nns, name, args } => {
                self.keyword(keyword);
                self.nns(nns);
                self.push("template ");
                self.symbol(name);
                self.arg_list(args, false);
                self.space_before_ph();
            }
            TypeView::PackExpansion { pattern, .. } => self.before(pattern),
            TypeView::PackIndexing { pattern, index, selected } => {
                if let Some(s) = selected {
                    self.print(s, "");
                } else {
                    self.print(pattern, "");
                    self.push("...[");
                    self.operand(index);
                    self.push("]");
                }
                self.space_before_ph();
            }
            TypeView::Decltype { expr, .. } => {
                self.push("decltype(");
                self.expr(expr);
                self.push(")");
                self.space_before_ph();
            }
            TypeView::DependentDecltype(p) => {
                self.push("decltype(");
                self.profile(p);
                self.push(")");
                self.space_before_ph();
            }
            TypeView::TypeOf { operand, unqual, .. } => {
                match operand {
                    TypeOfOperand::Expr(e) => {
                        self.push(if unqual { "typeof_unqual " } else { "typeof " });
                        self.expr(e);
                    }
                    TypeOfOperand::Profile(p) => {
                        self.push(if unqual { "typeof_unqual " } else { "typeof " });
                        self.profile(p);
                    }
                    TypeOfOperand::Type(ty) => {
                        self.push(if unqual { "typeof_unqual(" } else { "typeof(" });
                        self.print(ty, "");
                        self.push(")");
                    }
                }
                self.space_before_ph();
            }
            TypeView::UnaryTransform { kind, base, .. } => {
                self.push(kind.spelling());
                self.push("(");
                self.print(base, "");
                self.push(")");
                self.space_before_ph();
            }
            // A placeholder deduced from a type-dependent initializer
            // prints as written (clang's dependent `AutoType`).
            TypeView::Auto { deduced: Some(d), .. } if d != QualType::DEPENDENT => self.before(d),
            // A qualified placeholder (`N::S x(1)`) keeps its qualifier,
            // as clang's `ElaboratedType` around the deduced type does.
            TypeView::DeducedTemplateSpecialization { name, deduced: Some(d) } => {
                if let TemplateName::Qualified { nns, .. } = self.ctx.template_name_view(name) {
                    self.nns(nns);
                    self.with_scope(false, |p| p.before(d));
                } else {
                    self.before(d);
                }
            }
            TypeView::Auto { keyword, concept, args, .. } => {
                if let Some(c) = concept {
                    self.decl(c, false);
                    if let Some(a) = args
                        && !ctx.args(a).is_empty()
                    {
                        self.arg_list(a, false);
                    }
                    self.push(" ");
                }
                self.push(keyword.spelling());
                self.space_before_ph();
            }
            TypeView::DeducedTemplateSpecialization { name, deduced: None } => {
                self.template_name(self.ctx.dtst_display_name(name), Qual::AsWritten);
                self.space_before_ph();
            }
            TypeView::Attributed { kind, modified, .. } => {
                self.before(modified);
                if let Some(s) = nullability(kind) {
                    self.push(s);
                    self.space_before_ph();
                }
            }
            TypeView::Splice { operand, .. } => {
                self.push("[:");
                self.operand(operand);
                self.push(":]");
                self.space_before_ph();
            }
        }
    }

    fn after_node(&mut self, t: TypeId) {
        let ctx = self.ctx;
        match ctx.view(t) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                self.declarator_after(p);
            }
            TypeView::MemberPointer { pointee, .. } => self.declarator_after(pointee),
            TypeView::ConstantArray(a) => {
                self.push("[");
                self.num(a.size);
                self.push("]");
                self.after(a.elem);
            }
            TypeView::IncompleteArray(e) => {
                self.push("[]");
                self.after(e);
            }
            TypeView::VariableArray { elem, size } => {
                self.push("[");
                self.expr(size);
                self.push("]");
                self.after(elem);
            }
            TypeView::DependentSizedArray { elem, size } => {
                self.push("[");
                self.operand(size);
                self.push("]");
                self.after(elem);
            }
            TypeView::Function(_) => self.function_after(t),
            TypeView::Paren(inner) => {
                if self.paren_shown(inner) {
                    self.push(")");
                }
                self.after(inner);
            }
            TypeView::Complex(e) => self.after(e),
            TypeView::Vector { elem, count, kind } => {
                self.after(elem);
                if kind == VectorKind::Ext {
                    self.push(" __attribute__((ext_vector_type(");
                    self.num(count);
                    self.push(")))");
                }
            }
            TypeView::DependentVector { elem, size, kind } => {
                self.after(elem);
                if kind == VectorKind::Ext {
                    self.push(" __attribute__((ext_vector_type(");
                    self.operand(size);
                    self.push(")))");
                }
            }
            TypeView::ExtQual { base, .. } => self.after(base),
            TypeView::Elaborated { named, .. } => self.with_scope(false, |p| p.after(named)),
            TypeView::QualifiedArray { quals, base } => {
                self.after(QualType::new(base, quals.bits()));
            }
            TypeView::SubstTemplateTypeParm { replacement, .. } => {
                self.subst(replacement, Self::after);
            }
            TypeView::PackExpansion { pattern, .. } => {
                self.after(pattern);
                self.push("...");
            }
            TypeView::Auto { deduced: Some(d), .. } if d != QualType::DEPENDENT => self.after(d),
            TypeView::DeducedTemplateSpecialization { name, deduced: Some(d) } => {
                if let TemplateName::Qualified { .. } = self.ctx.template_name_view(name) {
                    self.with_scope(false, |p| p.after(d));
                } else {
                    self.after(d);
                }
            }
            TypeView::Attributed { kind, modified, equivalent } => {
                let saved = self.inside_cc_attr;
                self.inside_cc_attr = kind == TypeAttrKind::CallingConv;
                self.after(modified);
                self.inside_cc_attr = saved;
                match kind {
                    TypeAttrKind::Nonnull
                    | TypeAttrKind::Nullable
                    | TypeAttrKind::NullUnspecified
                    | TypeAttrKind::NullableResult
                    | TypeAttrKind::Other => {}
                    TypeAttrKind::LifetimeBound => self.push(" [[clang::lifetimebound]]"),
                    TypeAttrKind::NoDeref => self.push(" __attribute__((noderef))"),
                    TypeAttrKind::NoReturn => self.push(" __attribute__((noreturn))"),
                    TypeAttrKind::CallingConv => {
                        let cc = self.cc_of(equivalent);
                        let spelling = match cc {
                            CallConv::C | CallConv::Default => "cdecl",
                            cc => cc.attribute().unwrap_or("cdecl"),
                        };
                        self.push(" __attribute__((");
                        self.push(spelling);
                        self.push("))");
                    }
                }
            }
            _ => {}
        }
    }

    /// The calling convention of the function type `q` designates, looking
    /// through pointers (clang's `pcs` printing).
    fn cc_of(&self, q: QualType) -> CallConv {
        let ctx = self.ctx;
        let mut c = ctx.canonical(q);
        for _ in 0..MAX_DEPTH {
            if let Some(p) = ctx.proto_of(c.ty()) {
                return p.info().cc();
            }
            match ctx.pointee(c) {
                Some(p) => c = ctx.canonical(p),
                None => break,
            }
        }
        CallConv::Default
    }

    fn declarator_before(&mut self, pointee: QualType, sigil: &str) {
        let saved = self.empty_ph;
        self.empty_ph = false;
        self.before(pointee);
        if self.is_array_node(pointee) {
            self.push("(");
        }
        self.push(sigil);
        self.empty_ph = saved;
    }

    fn declarator_after(&mut self, pointee: QualType) {
        let saved = self.empty_ph;
        self.empty_ph = false;
        if self.is_array_node(pointee) {
            self.push(")");
        }
        self.after(pointee);
        self.empty_ph = saved;
    }

    fn function_before(&mut self, t: TypeId) {
        let Some(proto) = self.ctx.proto_of(t).copied() else { return };
        if proto.info().trailing_return() {
            self.push("auto ");
            if !self.empty_ph {
                self.push("(");
            }
        } else {
            let saved = self.empty_ph;
            self.empty_ph = false;
            self.before(proto.ret());
            self.empty_ph = saved;
            if !saved {
                self.push("(");
            }
        }
    }

    fn function_after(&mut self, t: TypeId) {
        let ctx = self.ctx;
        let Some(proto) = ctx.proto_of(t).copied() else { return };
        if !self.empty_ph {
            self.push(")");
        }
        let saved = self.empty_ph;
        self.empty_ph = false;
        self.push("(");
        let params = ctx.params(&proto);
        for (i, &p) in params.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.print(p, "");
        }
        if proto.variadic() {
            if !params.is_empty() {
                self.push(", ");
            }
            self.push("...");
        }
        self.push(")");
        let info = proto.info();
        if !self.inside_cc_attr
            && let Some(a) = info.cc().attribute()
        {
            self.push(" __attribute__((");
            self.push(a);
            self.push("))");
        }
        if info.noreturn() {
            self.push(" __attribute__((noreturn))");
        }
        let mq = info.this_quals();
        if !mq.is_empty() {
            self.push(" ");
            mq.write(&mut self.out);
        }
        match info.ref_qual() {
            RefQual::None => {}
            RefQual::LValue => self.push(" &"),
            RefQual::RValue => self.push(" &&"),
        }
        self.exc_spec(ctx.exc_spec(&proto));
        if info.trailing_return() {
            self.push(" -> ");
            self.print(proto.ret(), "");
        } else {
            self.after(proto.ret());
        }
        self.empty_ph = saved;
    }

    /// clang's `FunctionProtoType::printExceptionSpecification`.
    fn exc_spec(&mut self, exc: ExcSpec) {
        match exc {
            ExcSpec::None | ExcSpec::Deferred(_) => {}
            ExcSpec::Nothrow | ExcSpec::BasicNoexcept => self.push(" noexcept"),
            ExcSpec::DynamicNone => self.push(" throw()"),
            ExcSpec::NothrowAttr => self.push(" __attribute__((nothrow))"),
            ExcSpec::NoexceptExpr { expr, .. } => {
                self.push(" noexcept(");
                self.expr(expr);
                self.push(")");
            }
            ExcSpec::DependentNoexcept(p) => {
                self.push(" noexcept(");
                self.profile(p);
                self.push(")");
            }
            ExcSpec::Dynamic(list) => {
                self.push(" throw(");
                for (i, &ty) in self.ctx.type_list_types(list).iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.print(ty, "");
                }
                self.push(")");
            }
        }
    }

    fn parm_name(&mut self, depth: u32, index: u32) {
        self.push("type-parameter-");
        self.num(depth);
        self.push("-");
        self.num(index);
    }

    fn keyword(&mut self, k: TagKeyword) {
        if k != TagKeyword::None {
            self.push(k.spelling());
            self.push(" ");
        }
    }

    /// clang's `printTemplateId`.
    fn template_id(&mut self, name: TemplateNameId, args: ArgListId) {
        if self.diag_desugar {
            // clang's `printTemplateId` prints a specialization's name
            // with `Qualified::None`; its scope was the `ElaboratedType`
            // that desugaring removed (`'O::J<SA>' (aka 'J<ns::S>')`).
            self.with_scope(false, |p| p.template_name(name, Qual::None));
        } else {
            self.template_name(name, if self.scope { Qual::Fully } else { Qual::None });
        }
        let template = self.ctx.template_decl(name);
        let saved = self.in_tst_args;
        // The `aka` and canonical arguments have no `ElaboratedType`.
        self.in_tst_args = !self.diag_desugar && !self.scoped_args;
        if self.diag_desugar && self.c22 {
            // The written qualifier ([`Printer::diag_strip`]) leaves the
            // scope off; the desugared arguments print theirs.
            self.with_scope(true, |p| p.arg_list_prefix(args, false, usize::MAX, template, false));
        } else {
            self.arg_list_prefix(args, false, usize::MAX, template, false);
        }
        self.in_tst_args = saved;
    }

    /// [`Printer::diag_desugar`]: `q` with its sugar looked through, down
    /// to a node clang's `desugarForDiagnostic` keeps: a class template
    /// specialization (not an alias template's), or an undeduced `auto`
    /// with a type-dependent initializer (see [`desugar_top_for_print`]).
    fn diag_strip(&self, mut q: QualType) -> QualType {
        for _ in 0..MAX_DEPTH {
            match self.ctx.view(q.ty()) {
                TypeView::TemplateSpecialization { alias: false, .. } => return q,
                TypeView::Auto { deduced: Some(d), .. } if d == QualType::DEPENDENT => return q,
                // clang 22: a tag type or template-id keeps its written
                // keyword and qualifier, which are not sugar
                // (`'O::J<SA>' (aka 'O::J<ns::S>')`; `'O::S'` has no aka).
                TypeView::Elaborated { mut named, .. } if self.c22 => {
                    while let TypeView::Elaborated { named: inner, .. } =
                        self.ctx.view(split(self.ctx, named).0)
                    {
                        named = inner;
                    }
                    if matches!(
                        self.ctx.view(split(self.ctx, named).0),
                        TypeView::TemplateSpecialization { alias: false, .. }
                            | TypeView::Record(_)
                            | TypeView::Enum(_)
                    ) {
                        return q;
                    }
                }
                // clang substitutes canonical arguments: `N::S<int>` is a
                // class there, printed scoped (`'Box<N::S<int>, int>'`).
                TypeView::SubstTemplateTypeParm { replacement, .. }
                    if !self.ctx.is_dependent(replacement) =>
                {
                    return self.ctx.canonical(q);
                }
                _ => {}
            }
            let n = self.ctx.desugar_once(q);
            if n == q {
                return q;
            }
            q = n;
        }
        q
    }

    /// clang's `TemplateName::print`.
    fn template_name(&mut self, n: TemplateNameId, qual: Qual) {
        if self.depth >= MAX_DEPTH {
            self.push("...");
            return;
        }
        self.depth += 1;
        match self.ctx.template_name_view(n) {
            TemplateName::Template(d) => self.decl(d, qual == Qual::Fully || self.scope),
            TemplateName::Qualified { nns, template_kw, underlying } => {
                // clang keeps a template template argument written
                // `nns template name` as written (`Q<O::template J>`):
                // only a name without the keyword prints fully qualified.
                if qual == Qual::Fully && !template_kw {
                    self.template_name(underlying, Qual::Fully);
                } else {
                    if qual != Qual::None {
                        self.nns(nns);
                    }
                    if template_kw {
                        self.push("template ");
                    }
                    self.template_name(underlying, Qual::None);
                }
            }
            TemplateName::DependentIdentifier { nns, name } => {
                // clang prints a dependent name's qualifier unconditionally.
                self.nns(nns);
                self.push("template ");
                self.symbol(name);
            }
            TemplateName::DependentOperator { nns, op } => {
                // clang prints a dependent name's qualifier unconditionally.
                self.nns(nns);
                self.push("template operator ");
                let names = self.names;
                names.write_operator(op, &mut self.out);
            }
            TemplateName::TemplateTemplateParm { decl, depth, index, .. } => {
                let start = self.out.len();
                self.decl(decl, false);
                if self.out.len() == start {
                    self.push("template-parameter-");
                    self.num(depth);
                    self.push("-");
                    self.num(index);
                }
            }
            TemplateName::CanonTemplateTemplateParm { depth, index, .. } => {
                self.push("template-parameter-");
                self.num(depth);
                self.push("-");
                self.num(index);
            }
            TemplateName::SubstTemplateTemplateParm { replacement, .. } => {
                self.template_name(replacement, qual);
            }
            TemplateName::SubstTemplateTemplateParmPack { associated, index, .. } => {
                let names = self.names;
                names.write_template_param(associated, index, &mut self.out);
            }
            TemplateName::Using { underlying, .. } => self.template_name(underlying, qual),
            TemplateName::Overloaded(set) => {
                if let Some(&d) = self.ctx.name_decls(set).first() {
                    self.decl(d, false);
                }
            }
            TemplateName::Splice { expr, .. } => {
                self.push("[:");
                self.expr(expr);
                self.push(":]");
            }
            TemplateName::SpliceProfile(p) => {
                self.push("[:");
                self.profile(p);
                self.push(":]");
            }
        }
        self.depth -= 1;
    }

    /// clang's `NestedNameSpecifier::print`.
    fn nns(&mut self, n: NnsId) {
        if self.depth >= MAX_DEPTH {
            self.push("...::");
            return;
        }
        self.depth += 1;
        let view = self.ctx.nns_view(n);
        let prefix = match view {
            Nns::Namespace { prefix, .. }
            | Nns::NamespaceAlias { prefix, .. }
            | Nns::Type { prefix, .. }
            | Nns::Identifier { prefix, .. } => prefix,
            Nns::Global | Nns::Splice(_) => None,
        };
        if let Some(p) = prefix {
            self.nns(p);
        }
        match view {
            Nns::Global => {}
            Nns::Namespace { ns, .. } => {
                if self.names.is_anonymous_namespace(ns) {
                    self.depth -= 1;
                    return;
                }
                self.decl(ns, false);
            }
            Nns::NamespaceAlias { alias, .. } => self.decl(alias, false),
            Nns::Type { ty, template_kw, .. } => {
                if template_kw {
                    self.push("template ");
                }
                // `T::template X<A>::`: the component's own qualifier is
                // this one's prefix, already printed (clang's
                // `SuppressScope` for a type component).
                if let TypeView::DependentTemplateSpecialization { name, args, .. } =
                    self.ctx.view(ty.ty())
                {
                    if !template_kw {
                        self.push("template ");
                    }
                    self.symbol(name);
                    self.arg_list(args, false);
                    self.push("::");
                    self.depth -= 1;
                    return;
                }
                // A substituted parameter (`T::` with `T = ns::S`) prints
                // its canonical replacement under the same `SuppressScope`
                // policy: `S::` (clang's `NestedNameSpecifier::print`).
                let ty = match self.ctx.view(ty.ty()) {
                    TypeView::SubstTemplateTypeParm { replacement, .. }
                        if !self.ctx.is_dependent(replacement) =>
                    {
                        self.ctx.canonical(replacement)
                    }
                    _ => ty,
                };
                let saved_tag = self.suppress_tag;
                self.suppress_tag = true;
                // A written qualifier ([`Printer::diag_strip`] at clang 22)
                // prints as written: `'O::J<ns::S>'` for `O::J<SA>`.
                let saved_dd = self.diag_desugar;
                self.diag_desugar &= !self.c22;
                self.with_scope(false, |p| p.print(ty, ""));
                self.diag_desugar = saved_dd;
                self.suppress_tag = saved_tag;
            }
            Nns::Identifier { name, .. } => self.symbol(name),
            Nns::Splice(e) => {
                self.push("[:");
                self.expr(e);
                self.push(":]");
            }
        }
        self.push("::");
        self.depth -= 1;
    }

    /// clang's `printTemplateArgumentList` (`printTo`).
    fn arg_list(&mut self, l: ArgListId, is_pack: bool) {
        self.arg_list_prefix(l, is_pack, usize::MAX, None, false);
    }

    /// [`Self::arg_list`] of the first `len` arguments of `l`; the
    /// arguments of `template` when known, whose values print typed per
    /// [`TypeNames::arg_includes_type`] (`pack_typed` for the elements of
    /// a pack, which share one parameter).
    fn arg_list_prefix(
        &mut self,
        l: ArgListId,
        is_pack: bool,
        len: usize,
        template: Option<DeclId>,
        pack_typed: bool,
    ) {
        if self.depth >= MAX_DEPTH {
            self.push(if is_pack { "..." } else { "<...>" });
            return;
        }
        self.depth += 1;
        let ctx = self.ctx;
        if !is_pack {
            self.push("<");
        }
        let mut first = true;
        for (i, a) in ctx.args(l).iter().take(len).enumerate() {
            let start;
            let typed = if is_pack {
                pack_typed
            } else {
                template.is_some_and(|t| self.names.arg_includes_type(t, i))
            };
            match a.view() {
                ArgView::Pack(inner) => {
                    if !ctx.args(inner).is_empty() && !first {
                        self.push(", ");
                    }
                    start = self.out.len();
                    self.arg_list_prefix(inner, true, usize::MAX, None, typed);
                }
                ArgView::Value { ty, value } if typed => {
                    if !first {
                        self.push(", ");
                    }
                    start = self.out.len();
                    let names = self.names;
                    names.write_value_typed(ty, value, &mut self.out);
                }
                v => {
                    if !first {
                        self.push(", ");
                    }
                    start = self.out.len();
                    self.arg(v);
                }
            }
            if self.out.len() > start {
                // `<::foo>` would lex as the digraph `<:`.
                if first && self.out[start..].starts_with(':') {
                    self.out.insert(start, ' ');
                }
                first = false;
            }
        }
        if !is_pack {
            // clang's `SplitTemplateClosers`, set by the C policy (no
            // C++11): `P<P<char> >` ([`PrintOptions::split_closers`]).
            if self.split_closers && self.out.ends_with('>') {
                self.push(" ");
            }
            self.push(">");
        }
        self.depth -= 1;
    }

    /// clang's `TemplateArgument::print`.
    fn arg(&mut self, a: ArgView) {
        match a {
            ArgView::Null => self.push("(no value)"),
            ArgView::Type(q) => self.print(q, ""),
            ArgView::Value { ty, value } => {
                let names = self.names;
                let ty = if self.canon_args { self.ctx.canonical(ty) } else { ty };
                names.write_value(ty, value, &mut self.out);
            }
            ArgView::Template(n) => self.template_name(n, Qual::Fully),
            ArgView::TemplateExpansion { name, .. } => {
                self.template_name(name, Qual::AsWritten);
                self.push("...");
            }
            ArgView::Expr { expr, .. } => self.expr(expr),
            ArgView::Profile(p) => self.profile(p),
            ArgView::Pack(l) => self.arg_list(l, true),
        }
    }
}

fn nullability(kind: TypeAttrKind) -> Option<&'static str> {
    match kind {
        TypeAttrKind::Nonnull => Some(" _Nonnull"),
        TypeAttrKind::Nullable => Some(" _Nullable"),
        TypeAttrKind::NullUnspecified => Some(" _Null_unspecified"),
        TypeAttrKind::NullableResult => Some(" _Nullable_result"),
        _ => None,
    }
}
