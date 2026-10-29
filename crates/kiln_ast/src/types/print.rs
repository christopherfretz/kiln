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
use super::kinds::{CallConv, Quals, RefQual, TagKeyword, TypeAttrKind, TypeKind, VectorKind};
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
}

impl PrintOptions {
    /// The type as written (diagnostics' first string).
    pub const SUGARED: PrintOptions =
        PrintOptions { canonical: false, desugar: false, full_scope: false, suppress_tag: false };
    /// The canonical type with every name scoped (the `aka` string).
    pub const CANONICAL: PrintOptions =
        PrintOptions { canonical: true, desugar: false, full_scope: true, suppress_tag: false };
    /// The top-level-desugared type (the AST dump's second string).
    pub const DESUGARED: PrintOptions =
        PrintOptions { canonical: false, desugar: true, full_scope: false, suppress_tag: false };
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
    if opts.desugar {
        q = ctx.desugar_top(q);
        let (t, _) = split(ctx, q);
        // clang prints a bare RecordType (what desugaring an
        // ElaboratedType leaves) with its scope.
        if matches!(ctx.kind(t), TypeKind::Record | TypeKind::Enum) {
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
        inside_cc_attr: false,
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
        inside_cc_attr: false,
        depth: 0,
    };
    p.nns(n);
    *out = p.out;
}

/// Renders template-argument list `l` as written (clang's
/// `printTemplateArgumentList`: `<int, N>`), appending to `out`.
pub fn write_arg_list(ctx: &TypeContext, names: &dyn TypeNames, l: ArgListId, out: &mut String) {
    let mut p = Printer {
        ctx,
        names,
        out: std::mem::take(out),
        empty_ph: true,
        scope: false,
        suppress_tag: false,
        inside_cc_attr: false,
        depth: 0,
    };
    p.arg_list(l, false);
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
        inside_cc_attr: false,
        depth: 0,
    };
    p.arg(a.view());
    *out = p.out;
}

/// The diagnostic `aka` string: the canonical type printed with every
/// name scoped, when it differs textually from the type as written
/// (`'S' (aka 'ns::S')`).
pub fn aka(ctx: &TypeContext, names: &dyn TypeNames, q: QualType) -> Option<String> {
    let written = print_type(ctx, names, q, PrintOptions::SUGARED);
    let canon = print_type(ctx, names, q, PrintOptions::CANONICAL);
    (canon != written).then_some(canon)
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
    let differs = ctx.desugar_top(q) != q || desugared != written;
    (written, differs.then_some(desugared))
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
    while let TypeView::QualifiedArray { quals: qa, base } = ctx.view(t) {
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
    /// clang's `InsideCCAttribute`.
    inside_cc_attr: bool,
    depth: u32,
}

impl Printer<'_> {
    fn push(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn num(&mut self, n: impl Display) {
        self.out.push_str(&n.to_string());
    }

    fn space_before_ph(&mut self) {
        if !self.empty_ph {
            self.out.push(' ');
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
                TypeView::Auto { deduced: Some(d), .. } => t = ctx.canonical(d).ty(),
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
                self.push(b.name());
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
                // clang prints the class as a bare (scoped) type.
                self.with_scope(true, |p| p.print(QualType::unqual(class), ""));
                self.push("::*");
                self.empty_ph = saved;
            }
            TypeView::ConstantArray(a) => self.before(a.elem),
            TypeView::IncompleteArray(e)
            | TypeView::VariableArray { elem: e, .. }
            | TypeView::DependentSizedArray { elem: e, .. } => self.before(e),
            TypeView::Function(_) => self.function_before(t),
            TypeView::Record(d) => {
                self.decl(d, self.scope);
                if let Some(args) = self.names.specialization_args(d) {
                    self.arg_list(args, false);
                }
                self.space_before_ph();
            }
            TypeView::Enum(d) => {
                self.decl(d, self.scope);
                self.space_before_ph();
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
            TypeView::SubstTemplateTypeParm { replacement, .. } => self.before(replacement),
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
            TypeView::Auto { deduced: Some(d), .. }
            | TypeView::DeducedTemplateSpecialization { deduced: Some(d), .. } => self.before(d),
            TypeView::Auto { keyword, deduced: None, concept, args, .. } => {
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
                self.template_name(name, Qual::AsWritten);
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
            TypeView::SubstTemplateTypeParm { replacement, .. } => self.after(replacement),
            TypeView::PackExpansion { pattern, .. } => {
                self.after(pattern);
                self.push("...");
            }
            TypeView::Auto { deduced: Some(d), .. }
            | TypeView::DeducedTemplateSpecialization { deduced: Some(d), .. } => self.after(d),
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
        self.template_name(name, if self.scope { Qual::Fully } else { Qual::None });
        self.arg_list(args, false);
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
                if qual == Qual::Fully {
                    self.template_name(underlying, Qual::Fully);
                } else {
                    if qual == Qual::AsWritten {
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
                let saved_tag = self.suppress_tag;
                self.suppress_tag = true;
                self.with_scope(false, |p| p.print(ty, ""));
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
        for a in ctx.args(l) {
            let start;
            match a.view() {
                ArgView::Pack(inner) => {
                    if !ctx.args(inner).is_empty() && !first {
                        self.push(", ");
                    }
                    start = self.out.len();
                    self.arg_list(inner, true);
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
