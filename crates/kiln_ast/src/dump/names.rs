//! Names, values and value shapes over an [`Ast`]: the [`TypeNames`] and
//! [`PrintEnv`] implementations the dump prints types and values with.

use std::fmt::Write as _;

use kiln_source::SourceManager;
use kiln_source::symbol::Symbol;

use crate::Ast;
use crate::consts::{
    FloatKind, IntBits, IntStyle, Member, PrintEnv, PrintOptions as ValueOptions, PrintStyle,
    Scalar, Shape,
};
use crate::decl::{DeclFlags, DeclKind, DefinitionBits, TagKind};
use crate::expr::{CharKind, ExprFlags, ExprKind, ValueCategory};
use crate::ids::{ConstValueId, DeclId, ExprId, ListId, QualType, RawId, TypeId};
use crate::names::{DeclName, DeclNameView};
use crate::strings::{StringEncoding, push_char_literal};
use crate::types::{
    ArgView, FloatFormat, PrintOptions, TypeFlags, TypeNames, TypeView, write_arg_list,
    write_arg_list_canonical, write_arg_list_prefix, write_nns, write_nns_resolved, write_type,
};

/// Scope chains and expressions nested deeper than this print `...`
/// (no stack exhaustion on adversarial input).
const MAX_DEPTH: u32 = 64;

/// The [`TypeNames`] and [`PrintEnv`] of an [`Ast`], optionally with the
/// [`SourceManager`] its locations refer to (needed only for the
/// `(unnamed struct at F:L:C)` spelling of unnamed tags; without it they
/// print `(unnamed struct)`).
#[derive(Clone, Copy)]
pub struct AstNames<'a> {
    /// The AST.
    pub ast: &'a Ast,
    /// The source manager for unnamed-tag locations.
    pub sources: Option<&'a SourceManager>,
    /// How types written inside expressions print (`sizeof(T)`, casts,
    /// type traits): as written, or canonical for a failed
    /// `static_assert` requirement (clang's `PrintCanonicalTypes`).
    pub expr_types: PrintOptions,
    /// A closure type prints as `(lambda)` rather than `(anonymous class)`
    /// (clang 22's `NamedDecl::printNestedNameSpecifier`; set under
    /// `-fclang-compat=22`).
    pub closure_as_lambda: bool,
    /// With canonical expression types: a type qualifier prints as its
    /// canonical type with every name scoped (`n::tr<n::A, int>::value`),
    /// as clang 22 prints a failed requirement (`-fclang-compat=22`).
    pub scoped_nns: bool,
    /// The template arguments of a specialization in a scope chain print
    /// with every name scoped (`C<N::X>::`), as clang prints the
    /// canonical arguments it stores (`__PRETTY_FUNCTION__`).
    pub scoped_spec_args: bool,
}

impl std::fmt::Debug for AstNames<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AstNames").field("sources", &self.sources.is_some()).finish_non_exhaustive()
    }
}

impl<'a> AstNames<'a> {
    /// Names over `ast` without locations.
    pub fn new(ast: &'a Ast) -> Self {
        AstNames {
            ast,
            sources: None,
            expr_types: PrintOptions::SUGARED,
            closure_as_lambda: false,
            scoped_nns: false,
            scoped_spec_args: false,
        }
    }

    /// Names over `ast` printing unnamed tags with their location.
    pub fn with_sources(ast: &'a Ast, sources: &'a SourceManager) -> Self {
        AstNames {
            ast,
            sources: Some(sources),
            expr_types: PrintOptions::SUGARED,
            closure_as_lambda: false,
            scoped_nns: false,
            scoped_spec_args: false,
        }
    }

    /// These names, printing the types written in expressions canonical
    /// (clang's `PrintCanonicalTypes`, used for a failed `static_assert`
    /// requirement).
    #[must_use]
    pub fn canonical_expr_types(mut self) -> Self {
        self.expr_types = PrintOptions { canonical: true, ..PrintOptions::CANONICAL };
        self
    }

    /// Appends the unqualified name of `d` as the dump prints it
    /// (clang's `NamedDecl::printName`): identifiers, `operator+`, the
    /// class name for constructors, `~S`, `operator int`,
    /// `operator""_x`, `<deduction guide for S>`. Unnamed declarations
    /// append nothing.
    pub fn write_name(&self, d: DeclId, out: &mut String) {
        let n = self.ast.decls.get(d);
        // A conversion function template specialization keeps the
        // pattern's name (`operator T`, which the mangling needs); clang
        // names it by its substituted target type (`operator int`).
        if let Some(&q) = self.ast.decls.cold.conversion_spec_target.get(d) {
            out.push_str("operator ");
            write_type(&self.ast.types, self, q, PrintOptions::SUGARED, out);
            return;
        }
        self.write_decl_name_value(n.name(), out);
    }

    /// Appends `name`.
    pub fn write_decl_name_value(&self, name: DeclName, out: &mut String) {
        match self.ast.names.view(name) {
            DeclNameView::Empty => {}
            DeclNameView::Identifier(s) => out.push_str(self.ast.symbols().as_str(s)),
            DeclNameView::Operator(op) => {
                let sp = op.spelling();
                out.push_str("operator");
                if sp.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
                    out.push(' ');
                }
                out.push_str(sp);
            }
            DeclNameView::Constructor(t) => self.write_class_type(t, out),
            DeclNameView::Destructor(t) => {
                out.push('~');
                self.write_class_type(t, out);
            }
            DeclNameView::Conversion(q) => {
                out.push_str("operator ");
                write_type(&self.ast.types, self, q, PrintOptions::SUGARED, out);
            }
            DeclNameView::LiteralOperator(s) => {
                out.push_str("operator\"\"");
                out.push_str(self.ast.symbols().as_str(s));
            }
            DeclNameView::DeductionGuide(t) => {
                out.push_str("<deduction guide for ");
                self.write_decl_name(t, false, out);
                out.push('>');
            }
        }
    }

    fn write_class_type(&self, t: TypeId, out: &mut String) {
        // clang `printCXXConstructorDestructorName`: a class template
        // specialization prints its bare name (`Base`, not `Base<int>`).
        if let Some(d) = self.ast.types.as_record(QualType::unqual(t))
            && self.ast.spec_info(d).is_some()
        {
            self.write_decl_name_value(self.ast.decls.get(d).name(), out);
            return;
        }
        // The pattern of a class template: clang's class type is the
        // injected-class-name `X<P...>`, printed with the parameters'
        // names (`Base<T>`, `~Tuple<Ts...>`).
        if let Some(d) = self.ast.types.as_record(QualType::unqual(t))
            && let Some(tmpl) = self.ast.described_template(d)
            && let Some(info) = self.ast.template_info(tmpl)
        {
            self.write_decl_name_value(self.ast.decls.get(d).name(), out);
            out.push('<');
            let list = self.ast.templates.param_list(info.params);
            for (i, q) in self.ast.lists.iter::<DeclId>(list.params).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let parm = self.ast.template_parm(q).copied();
                if let Some(n) = self.ast.decls.get(q).name().as_ident() {
                    out.push_str(self.ast.symbols().as_str(n));
                } else {
                    let (depth, index) = parm.map_or((0, 0), |p| (p.depth, p.index));
                    let _ = write!(out, "type-parameter-{depth}-{index}");
                }
                if parm.is_some_and(|p| p.pack) {
                    out.push_str("...");
                }
            }
            out.push('>');
            return;
        }
        write_type(&self.ast.types, self, QualType::unqual(t), PrintOptions::SUGARED, out);
    }

    /// True if `d` has no name.
    pub fn is_unnamed(&self, d: DeclId) -> bool {
        matches!(self.ast.names.view(self.ast.decls.get(d).name()), DeclNameView::Empty)
    }

    /// Appends the qualified name of `d` as clang's
    /// `NamedDecl::getQualifiedNameAsString` (spec/ast-dump.md §1): the
    /// enclosing scopes outermost first, skipping `extern "C"`, `export`,
    /// unscoped enumerations and inline namespaces; anonymous namespaces
    /// print `(anonymous namespace)`, unnamed classes `(anonymous
    /// struct)`, enclosing functions `f(int, char *)`. A declaration
    /// directly inside a function prints its bare name.
    pub fn write_qualified_name(&self, d: DeclId, out: &mut String) {
        self.write_scope_prefix(d, false, out);
        self.write_component(d, out);
    }

    /// Appends the name `-fdump-ast-filter` matches and prints after
    /// `Dumping` (clang `NamedDecl::printQualifiedName`, spec/ast-dump.md
    /// §1): [`Self::write_qualified_name`], but a class template
    /// specialization context prints its arguments (`n::X<int>::N`) and a
    /// partial specialization its canonical ones (`X<type-parameter-0-0
    /// *, true>::N`). `parm_of` is the pattern (or partial
    /// specialization) of the template whose parameter `d` is: clang's
    /// context of a class template's parameter is its class
    /// (`n::X::T`), of a function template's its function (`T`); the
    /// parameters of other templates are named by `d`'s own context.
    pub fn write_filter_name(&self, d: DeclId, parm_of: Option<DeclId>, out: &mut String) {
        match parm_of.map(|o| (o, self.ast.decls.get(o).kind())) {
            Some((_, k)) if k.is_function() => {}
            Some((o, DeclKind::CXXRecord)) => self.write_scope_chain(o, 0, true, out),
            _ => self.write_scope_prefix(d, true, out),
        }
        self.write_component(d, out);
    }

    /// The arguments partial specialization `ctx` prints with as a
    /// context (clang `printNestedNameSpecifier` prints `getTemplateArgs()`
    /// without the trailing arguments `isSubstitutedDefaultArgument`
    /// finds equal to their default): its canonical arguments and how
    /// many print. An argument is dropped when its parameter's default is
    /// non-dependent and equal to it, or is an earlier parameter of the
    /// template (`class U = T`) whose argument equals it; any other
    /// dependent default keeps it (spec/ast-dump.md §1).
    fn partial_print_args(&self, ctx: DeclId) -> Option<(crate::ids::ArgListId, usize)> {
        if self.ast.decls.get(ctx).kind() != DeclKind::CXXRecord {
            return None;
        }
        let (_, pi) = self.ast.partial_spec_info(ctx)?;
        let types = &self.ast.types;
        let args = types.args(pi.canonical);
        let params: Vec<DeclId> = self
            .ast
            .template_info(pi.template)
            .map(|i| self.ast.lists.iter::<DeclId>(self.ast.templates.param_list(i.params).params))
            .into_iter()
            .flatten()
            .collect();
        let mut len = args.len();
        if params.len() != len {
            return Some((pi.canonical, len));
        }
        while len > 0 {
            let p = params[len - 1];
            if self.ast.template_parm(p).is_none_or(|x| x.pack) {
                break;
            }
            let Some(&def) = self.ast.templates.defaults.get(p) else { break };
            let def = types.canonical_arg(def);
            let arg = types.canonical_arg(args[len - 1]);
            let equal = if types.arg_flags(def).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
                // `class U = T`: the default is parameter `j` of this
                // template.
                match def.view() {
                    ArgView::Type(q) => match types.view(q.ty()) {
                        TypeView::CanonTemplateTypeParm { depth, index, pack: false }
                            if (index as usize) < len - 1
                                && u32::from(self.param_depth(p)) == depth =>
                        {
                            types.canonical_arg(args[index as usize]) == arg
                        }
                        _ => false,
                    },
                    _ => false,
                }
            } else if let (ArgView::Expr { expr, .. }, ArgView::Value { value, .. }) =
                (def.view(), arg.view())
            {
                // A literal default (`bool B = false`) is kept unconverted.
                let v = match self.ast.consts.scalar(value) {
                    Some(Scalar::Int { bits, .. }) => bits.to_u128(),
                    Some(Scalar::Bool { value, .. }) => Some(u128::from(value)),
                    _ => None,
                };
                v.is_some() && v == self.expr_uint(expr).map(u128::from)
            } else {
                def == arg
            };
            if !equal {
                break;
            }
            len -= 1;
        }
        Some((pi.canonical, len))
    }

    /// The depth of template parameter `p`.
    fn param_depth(&self, p: DeclId) -> u16 {
        self.ast.template_parm(p).map_or(u16::MAX, |x| x.depth)
    }

    /// Appends the scopes of `d`'s qualified name, each followed by `::`
    /// (empty directly inside a function); with `spec_args`, class
    /// template specializations print their arguments without trailing
    /// defaults (clang `NamedDecl::printNestedNameSpecifier`, `A<int>::`).
    pub fn write_scope_prefix(&self, d: DeclId, spec_args: bool, out: &mut String) {
        let n = self.ast.decls.get(d);
        if let Some(ctx) = n.ctx()
            && !self.ast.decls.get(ctx).kind().is_function()
        {
            self.write_scope_chain(ctx, 0, spec_args, out);
        }
    }

    /// Appends `ctx` and its parents, each followed by `::`.
    fn write_scope_chain(&self, ctx: DeclId, depth: u32, spec_args: bool, out: &mut String) {
        if depth > MAX_DEPTH {
            out.push_str("...::");
            return;
        }
        let n = self.ast.decls.get(ctx);
        let kind = n.kind();
        if kind == DeclKind::TranslationUnit {
            return;
        }
        if let Some(parent) = n.ctx() {
            self.write_scope_chain(parent, depth + 1, spec_args, out);
        }
        let skip = match kind {
            DeclKind::LinkageSpec | DeclKind::Export => true,
            DeclKind::Enum => self.ast.is_transparent(ctx),
            DeclKind::Namespace => self
                .ast
                .decls
                .namespace(ctx)
                .is_some_and(super::super::decl::NamespaceData::is_inline),
            _ => false,
        };
        if !skip {
            self.write_component(ctx, out);
            if spec_args && !self.is_unnamed(ctx) {
                if let Some(args) = self.ast.spec_print_args(ctx) {
                    if self.scoped_spec_args {
                        crate::types::write_arg_list_scoped_of(
                            &self.ast.types,
                            self,
                            ctx,
                            args,
                            out,
                        );
                    } else {
                        crate::types::write_arg_list_of(&self.ast.types, self, ctx, args, out);
                    }
                } else if let Some((args, len)) = self.partial_print_args(ctx) {
                    // A partial specialization's arguments print scoped
                    // too, as the specialization's (WP-753).
                    if self.scoped_spec_args {
                        crate::types::write_arg_list_prefix_scoped(
                            &self.ast.types,
                            self,
                            args,
                            len,
                            out,
                        );
                    } else {
                        write_arg_list_prefix(&self.ast.types, self, args, len, out);
                    }
                }
            }
            if kind.is_function() {
                self.write_function_scope(ctx, out);
            }
            out.push_str("::");
        }
    }

    /// One component of a qualified name.
    fn write_component(&self, d: DeclId, out: &mut String) {
        let n = self.ast.decls.get(d);
        let kind = n.kind();
        if self.is_unnamed(d) {
            if let Some(td) = self.ast.decls.typedef_for_anon(d) {
                self.write_name(td, out);
                return;
            }
            match kind {
                DeclKind::Namespace => out.push_str("(anonymous namespace)"),
                DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization => {
                    if self.closure_as_lambda && self.ast.closure_lambda(d).is_some() {
                        out.push_str("(lambda)");
                        return;
                    }
                    let tag = self.ast.decls.record(d).map_or(TagKind::Struct, |r| r.tag);
                    out.push_str("(anonymous ");
                    out.push_str(tag.keyword());
                    out.push(')');
                }
                DeclKind::Enum => out.push_str("(anonymous enum)"),
                _ => {}
            }
            return;
        }
        self.write_name(d, out);
    }

    /// Appends the scope prefix of a function used as a scope:
    /// `f(char, int *)`.
    fn write_function_scope(&self, f: DeclId, out: &mut String) {
        out.push('(');
        let proto = self.ast.decls.get(f).ty().and_then(|t| self.ast.types.proto(t));
        if let Some(p) = proto {
            let params = self.ast.types.params(p);
            for (i, &q) in params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_type(&self.ast.types, self, q, PrintOptions::SUGARED, out);
            }
            if p.variadic() {
                out.push_str(if params.is_empty() { "..." } else { ", ..." });
            }
        }
        out.push(')');
    }

    /// Appends the name of tag `d` when it is unnamed:
    /// `(unnamed struct at F:L:C)`, `(anonymous union at F:L:C)`,
    /// `(lambda at F:L:C)`, `(unnamed enum at F:L:C)`; without a source
    /// manager the ` at ...` part is omitted.
    fn write_unnamed_tag(&self, d: DeclId, out: &mut String) {
        let n = self.ast.decls.get(d);
        out.push('(');
        if let Some(r) = self.ast.decls.record(d) {
            if r.defn.contains(DefinitionBits::LAMBDA) {
                out.push_str("lambda");
            } else {
                out.push_str(if r.defn.contains(DefinitionBits::ANONYMOUS) {
                    "anonymous "
                } else {
                    "unnamed "
                });
                out.push_str(r.tag.keyword());
            }
        } else {
            out.push_str("unnamed enum");
        }
        if let Some(sm) = self.sources
            && let Some(p) = sm.presumed_line_col(n.loc())
        {
            let _ = write!(out, " at {}:{}:{}", p.file_name, p.line, p.col);
        }
        out.push(')');
    }

    /// The enclosing scopes of a type name (clang's
    /// `TypePrinter::AppendScope`).
    fn append_scope(&self, ctx: Option<DeclId>, depth: u32, out: &mut String) {
        let Some(ctx) = ctx else { return };
        if depth > MAX_DEPTH {
            return;
        }
        let n = self.ast.decls.get(ctx);
        let kind = n.kind();
        if kind == DeclKind::TranslationUnit || kind.is_function() {
            return;
        }
        self.append_scope(n.ctx(), depth + 1, out);
        match kind {
            DeclKind::Namespace => {
                if self
                    .ast
                    .decls
                    .namespace(ctx)
                    .is_some_and(super::super::decl::NamespaceData::is_inline)
                {
                    return;
                }
                if self.is_unnamed(ctx) {
                    out.push_str("(anonymous namespace)::");
                } else {
                    self.write_name(ctx, out);
                    out.push_str("::");
                }
            }
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::Enum
                if !self.is_unnamed(ctx) =>
            {
                self.write_name(ctx, out);
                if let Some(args) = self.ast.spec_print_args(ctx) {
                    crate::types::write_arg_list_of(&self.ast.types, self, ctx, args, out);
                }
                out.push_str("::");
            }
            _ => {}
        }
    }

    /// [`Self::append_scope`] with each enclosing class template
    /// specialization's full converted argument list, canonical
    /// ([`TypeNames::write_converted_scope`]).
    fn append_converted_scope(&self, ctx: Option<DeclId>, depth: u32, out: &mut String) {
        let Some(ctx) = ctx else { return };
        if depth > MAX_DEPTH {
            return;
        }
        let n = self.ast.decls.get(ctx);
        let kind = n.kind();
        if kind == DeclKind::TranslationUnit || kind.is_function() {
            return;
        }
        self.append_converted_scope(n.ctx(), depth + 1, out);
        match kind {
            DeclKind::Namespace => {
                let inline = self
                    .ast
                    .decls
                    .namespace(ctx)
                    .is_some_and(super::super::decl::NamespaceData::is_inline);
                if inline {
                    return;
                }
                if self.is_unnamed(ctx) {
                    out.push_str("(anonymous namespace)::");
                } else {
                    self.write_name(ctx, out);
                    out.push_str("::");
                }
            }
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::Enum
                if !self.is_unnamed(ctx) =>
            {
                self.write_name(ctx, out);
                if let Some(args) = self.converted_args(ctx) {
                    write_arg_list_canonical(&self.ast.types, self, args, out);
                }
                out.push_str("::");
            }
            _ => {}
        }
    }

    /// The `FieldDecl`s of record definition `r`, in declaration order.
    fn fields(&self, r: DeclId) -> impl Iterator<Item = DeclId> + '_ {
        self.ast.members(r).filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
    }

    /// `sizeof(q)` in bytes on the AST's target, if known.
    pub fn size_of(&self, q: QualType) -> Option<u64> {
        let types = &self.ast.types;
        match types.canonical_view(q) {
            TypeView::Builtin(b) => {
                let (s, _) = types.target().builtin_size_align(b);
                (s != 0).then_some(u64::from(s))
            }
            TypeView::BitInt { width, .. } => {
                let bytes = u64::from(width).div_ceil(8);
                Some(bytes.next_power_of_two().max(1))
            }
            TypeView::Pointer(_)
            | TypeView::LValueRef(_)
            | TypeView::RValueRef(_)
            | TypeView::MemberPointer { .. } => {
                let p = u64::from(types.target().pointer_bytes);
                (p != 0).then_some(p)
            }
            TypeView::ConstantArray(a) => self.size_of(a.elem).map(|s| s * a.size),
            TypeView::Record(d) => self.ast.layouts.get(self.ast.first(d)).map(|l| l.size),
            TypeView::Enum(d) => {
                self.ast.decls.enumeration(d)?.int_type.and_then(|t| self.size_of(t))
            }
            _ => None,
        }
    }

    /// The value of a constant integer expression (a bit-field width, a
    /// literal default template argument).
    fn expr_uint(&self, mut e: ExprId) -> Option<u64> {
        let exprs = &self.ast.exprs;
        for _ in 0..MAX_DEPTH {
            let n = exprs.get(e);
            match n.kind() {
                ExprKind::ConstantExpr => {
                    if let Some(v) = exprs.constant_value(e) {
                        return match self.ast.consts.scalar(v)? {
                            Scalar::Int { bits, .. } => bits.to_u128().map(|x| x as u64),
                            _ => None,
                        };
                    }
                    e = exprs.operand(e);
                }
                ExprKind::IntegerLiteral => return exprs.int_value(e),
                ExprKind::CXXBoolLiteralExpr => return Some(u64::from(exprs.bool_value(e))),
                ExprKind::CharacterLiteral => return Some(u64::from(exprs.char_value(e))),
                ExprKind::ParenExpr => e = exprs.operand(e),
                k if k.is_cast() => e = exprs.operand(e),
                _ => return None,
            }
        }
        None
    }

    fn int_shape(&self, b: crate::types::BuiltinType) -> Shape {
        let t = self.ast.types.target();
        let (size, _) = t.builtin_size_align(b);
        if size == 0 {
            return Shape::Unknown;
        }
        let width = u32::from(size) * 8;
        let id = b as u8;
        let (signed, style) = match id {
            // char
            3 => (t.char_signed, IntStyle::Char(StringEncoding::Ordinary)),
            // signed char, short; unsigned char, unsigned short; __int128
            4 => (true, IntStyle::SmallChar),
            5 => (false, IntStyle::SmallChar),
            10 | 18 => (true, IntStyle::Cast),
            11 | 19 => (false, IntStyle::Cast),
            // wchar_t
            6 => (
                t.wchar_signed,
                IntStyle::Char(if size == 2 {
                    StringEncoding::Wide16
                } else {
                    StringEncoding::Wide32
                }),
            ),
            7 => (false, IntStyle::Char(StringEncoding::Utf8)),
            8 => (false, IntStyle::Char(StringEncoding::Utf16)),
            9 => (false, IntStyle::Char(StringEncoding::Utf32)),
            12 => (true, IntStyle::Plain { suffix: "" }),
            13 => (false, IntStyle::Plain { suffix: "U" }),
            14 => (true, IntStyle::Plain { suffix: "L" }),
            15 => (false, IntStyle::Plain { suffix: "UL" }),
            16 => (true, IntStyle::Plain { suffix: "LL" }),
            17 => (false, IntStyle::Plain { suffix: "ULL" }),
            _ => return Shape::Unknown,
        };
        Shape::Int { width, signed, style }
    }

    fn float_kind(&self, b: crate::types::BuiltinType) -> Option<FloatKind> {
        Some(match b as u8 {
            20 | 26 => FloatKind::Half,
            21 => FloatKind::BFloat16,
            22 | 48 => FloatKind::Single,
            23 | 49 | 51 => FloatKind::Double,
            24 | 52 => match self.ast.types.target().long_double {
                FloatFormat::IeeeHalf => FloatKind::Half,
                FloatFormat::BFloat => FloatKind::BFloat16,
                FloatFormat::IeeeSingle => FloatKind::Single,
                FloatFormat::IeeeDouble => FloatKind::Double,
                FloatFormat::X87Extended => FloatKind::X87Extended,
                FloatFormat::IeeeQuad => FloatKind::Quad,
                FloatFormat::PpcDoubleDouble => return None,
            },
            25 | 50 => FloatKind::Quad,
            _ => return None,
        })
    }

    fn shape_at(&self, q: QualType, depth: u32) -> Shape {
        if depth > MAX_DEPTH {
            return Shape::Unknown;
        }
        let types = &self.ast.types;
        match types.canonical_view(q) {
            TypeView::Builtin(b) => match b as u8 {
                2 => Shape::Bool,
                27 => Shape::NullPtr,
                28 => Shape::Reflection,
                3..=19 => self.int_shape(b),
                20..=26 => self.float_kind(b).map_or(Shape::Unknown, Shape::Float),
                _ => Shape::Unknown,
            },
            TypeView::BitInt { signed, width } => {
                Shape::Int { width, signed, style: IntStyle::Cast }
            }
            TypeView::Pointer(p) => Shape::Pointer { pointee: Some(p), reference: false },
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                Shape::Pointer { pointee: Some(p), reference: true }
            }
            TypeView::MemberPointer { .. } => Shape::MemberPointer,
            TypeView::ConstantArray(a) => match self.size_of(a.elem) {
                Some(stride) => Shape::Array { elem: a.elem, stride, len: a.size },
                None => Shape::Unknown,
            },
            TypeView::Record(d) => self.record_shape(d),
            TypeView::Enum(d) => {
                let Some(int) = self.ast.decls.enumeration(d).and_then(|e| e.int_type) else {
                    return Shape::Unknown;
                };
                match self.shape_at(int, depth + 1) {
                    Shape::Int { width, signed, .. } => {
                        Shape::Int { width, signed, style: IntStyle::Enum }
                    }
                    // An enumeration with underlying type `bool` is still
                    // an enumeration: its values print as enumerators
                    // (`B::Nay`, `(B)1`), not `true`/`false` (clang
                    // `TemplateArgument::print`; magic_enum's names).
                    Shape::Bool => Shape::Int {
                        width: u32::from(
                            self.ast
                                .types
                                .target()
                                .builtin_size_align(crate::types::BuiltinType::Bool)
                                .0,
                        ) * 8,
                        signed: false,
                        style: IntStyle::Enum,
                    },
                    s => s,
                }
            }
            _ => Shape::Unknown,
        }
    }

    fn record_shape(&self, d: DeclId) -> Shape {
        let Some(def) = self.ast.definition(d) else { return Shape::Unknown };
        let Some(layout) = self.ast.layouts.get(self.ast.first(def)) else {
            return Shape::Unknown;
        };
        let union = self.ast.decls.record(def).is_some_and(|r| r.tag == TagKind::Union);
        let mut members = Vec::new();
        for &(base, off) in &*layout.bases {
            let Some(ty) = self.ast.decls.get(base).ty() else { return Shape::Unknown };
            members.push(Member {
                name: String::new(),
                ty,
                offset_bits: off * 8,
                size: self.size_of(ty).unwrap_or(0),
                bit_width: None,
                field: None,
            });
        }
        for (i, f) in self.fields(def).enumerate() {
            let Some(&offset_bits) = layout.fields.get(i) else { return Shape::Unknown };
            let Some(ty) = self.ast.decls.get(f).ty() else { return Shape::Unknown };
            let bit_width = self
                .ast
                .decls
                .field(f)
                .and_then(|fd| fd.bit_width)
                .and_then(|w| self.expr_uint(w))
                .map(|w| w as u32);
            let mut name = String::new();
            self.write_name(f, &mut name);
            members.push(Member {
                name,
                ty,
                offset_bits,
                size: if bit_width.is_some() { 0 } else { self.size_of(ty).unwrap_or(0) },
                bit_width,
                field: Some(f),
            });
        }
        Shape::Record { members, union }
    }

    // ---------------------------------------------------------------
    // expressions (clang's `Expr::printPretty`, bounded)
    // ---------------------------------------------------------------

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per expression kind, as clang's StmtPrinter"
    )]
    fn expr(&self, e: ExprId, depth: u32, out: &mut String) {
        if depth > MAX_DEPTH {
            out.push_str("...");
            return;
        }
        let ast = self.ast;
        let exprs = &ast.exprs;
        let n = exprs.get(e);
        let d = depth + 1;
        match n.kind() {
            ExprKind::IntegerLiteral => {
                match exprs.int_value(e) {
                    Some(v) => {
                        let _ = write!(out, "{v}");
                    }
                    None => {
                        if let Some(c) = exprs.literal_const(e) {
                            self.write_dump_value(c, out);
                        }
                    }
                }
                let suffix = match ast.types.builtin(n.ty()).map(|b| b as u8) {
                    Some(13) => "U",
                    Some(14) => "L",
                    Some(15) => "UL",
                    Some(16) => "LL",
                    Some(17) => "ULL",
                    _ => "",
                };
                out.push_str(suffix);
            }
            // clang `PrintFloatingLiteral`: `APFloat::toString` (round-trip
            // digits, `1`, `2.5E+10`), a `.` after a bare integer, and the
            // type's suffix (none for `double` and `__fp16`).
            ExprKind::FloatingLiteral => {
                let c = exprs.literal_const(e);
                if let Some(Scalar::Float { kind, bits, .. }) = c.and_then(|c| ast.consts.scalar(c))
                {
                    let s = crate::consts::fltstr::to_string(kind, bits, 0);
                    let bare = s.bytes().all(|b| b == b'-' || b.is_ascii_digit());
                    out.push_str(&s);
                    if bare {
                        out.push('.');
                    }
                    out.push_str(match ast.types.builtin(n.ty()) {
                        Some(crate::types::BuiltinType::Float) => "F",
                        Some(crate::types::BuiltinType::LongDouble) => "L",
                        Some(crate::types::BuiltinType::Float16) => "F16",
                        Some(crate::types::BuiltinType::Float128) => "Q",
                        _ => "",
                    });
                } else if let Some(c) = c {
                    self.write_dump_value(c, out);
                }
            }
            ExprKind::CharacterLiteral => {
                let kind = exprs.char_kind(e);
                out.push_str(match kind {
                    CharKind::Ascii => "",
                    CharKind::Wide => "L",
                    CharKind::Utf8 => "u8",
                    CharKind::Utf16 => "u",
                    CharKind::Utf32 => "U",
                });
                out.push('\'');
                let ascii = matches!(kind, CharKind::Ascii);
                push_char_literal(exprs.char_value(e), ascii, out);
                out.push('\'');
            }
            ExprKind::StringLiteral => ast.strings.write_literal(exprs.string_lit(e), out),
            ExprKind::CXXBoolLiteralExpr => {
                out.push_str(if exprs.bool_value(e) { "true" } else { "false" });
            }
            ExprKind::CXXNullPtrLiteralExpr => out.push_str("nullptr"),
            ExprKind::GNUNullExpr => out.push_str("__null"),
            ExprKind::AddrLabelExpr => {
                out.push_str("&&");
                self.write_name(exprs.addr_label(e), out);
            }
            // clang's `VisitDeclRefExpr`: the qualifier as written, then
            // the name.
            ExprKind::DeclRefExpr => {
                let extra = exprs.decl_ref_extra(e);
                if self.expr_types.canonical {
                    // clang's `FailedBooleanConditionPrinterHelper`: a
                    // qualified name prints its qualifier's template
                    // arguments resolved, then the name with a variable
                    // template specialization's arguments; an unqualified
                    // one its explicit arguments, canonical.
                    let d0 = exprs.decl(e);
                    if let Some(q) = extra.and_then(|x| x.qualifier) {
                        write_nns_resolved(&ast.types, self, q, self.scoped_nns, out);
                        self.write_name(d0, out);
                        if ast.decls.get(d0).kind() == DeclKind::Var
                            && let Some((_, info)) = ast.spec_info(d0)
                        {
                            write_arg_list_canonical(
                                &ast.types,
                                self,
                                ast.types.canonical_args(info.args),
                                out,
                            );
                        }
                    } else {
                        self.write_name(d0, out);
                        if let Some(l) = extra.and_then(|x| x.template_args) {
                            write_arg_list_canonical(&ast.types, self, l, out);
                        }
                    }
                    return;
                }
                if let Some(q) = extra.and_then(|x| x.qualifier) {
                    write_nns(&ast.types, self, q, out);
                }
                if ast.templates.template_kw.contains(e) {
                    out.push_str("template ");
                }
                self.write_name(exprs.decl(e), out);
                // clang `VisitDeclRefExpr`: explicit template arguments as
                // written (`n::same<n::A, n::A>`).
                if let Some(l) = exprs.decl_ref_extra(e).and_then(|x| x.template_args) {
                    write_arg_list(&ast.types, self, l, out);
                }
            }
            ExprKind::PredefinedExpr => out.push_str(exprs.predefined_kind(e).spelling()),
            ExprKind::SourceLocExpr => {
                out.push_str(exprs.source_loc_kind(e).spelling());
                out.push_str("()");
            }
            ExprKind::MemberExpr => {
                // clang `VisitMemberExpr`: an anonymous struct or union
                // member prints nothing, nor does the separator after it
                // (`h.s` for `h.<anonymous union>.s`).
                let anon = |m: DeclId| ast.decls.get(m).name().is_empty();
                let base = exprs.operand(e);
                self.expr(base, d, out);
                let after_anon =
                    exprs.get(base).kind() == ExprKind::MemberExpr && anon(exprs.decl(base));
                if !after_anon {
                    out.push_str(if n.has(ExprFlags::ARROW) { "->" } else { "." });
                }
                if anon(exprs.decl(e)) {
                    return;
                }
                self.write_name(exprs.decl(e), out);
                // clang `VisitMemberExpr`: explicit template arguments as
                // written (`s.get<0>`).
                if let Some(l) = exprs.decl_ref_extra(e).and_then(|x| x.template_args) {
                    write_arg_list(&ast.types, self, l, out);
                }
            }
            ExprKind::UnaryOperator => {
                let op = exprs.unary_op(e);
                if op.is_postfix() {
                    self.expr(exprs.operand(e), d, out);
                    out.push_str(op.spelling());
                } else {
                    out.push_str(op.spelling());
                    if op.spelling().as_bytes()[0].is_ascii_alphabetic() {
                        out.push(' ');
                    }
                    self.expr(exprs.operand(e), d, out);
                }
            }
            ExprKind::BinaryOperator => {
                let [l, r] = exprs.operands(e);
                self.expr(l, d, out);
                out.push(' ');
                out.push_str(exprs.binary_op(e).spelling());
                out.push(' ');
                self.expr(r, d, out);
            }
            ExprKind::CompoundAssignOperator => {
                self.expr(ExprId::new(n.a()), d, out);
                out.push(' ');
                out.push_str(exprs.binary_op(e).spelling());
                out.push(' ');
                self.expr(exprs.compound_assign(e).rhs, d, out);
            }
            // clang `StmtPrinter::VisitChooseExpr`.
            ExprKind::ChooseExpr => {
                out.push_str("__builtin_choose_expr(");
                for (i, c) in exprs.choose_operands(e).into_iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.expr(c, d, out);
                }
                out.push(')');
            }
            ExprKind::ConditionalOperator => {
                let [t, f] = exprs.cond_arms(e);
                self.expr(ExprId::new(n.a()), d, out);
                out.push_str(" ? ");
                self.expr(t, d, out);
                out.push_str(" : ");
                self.expr(f, d, out);
            }
            ExprKind::ArraySubscriptExpr => {
                let [l, r] = exprs.operands(e);
                self.expr(l, d, out);
                out.push('[');
                self.expr(r, d, out);
                out.push(']');
            }
            ExprKind::ParenExpr => {
                out.push('(');
                self.expr(exprs.operand(e), d, out);
                out.push(')');
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => {
                self.expr(exprs.callee(e), d, out);
                self.expr_list(exprs.call_args(e), '(', ')', d, out);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                self.expr(exprs.rewritten_semantic(e), depth, out);
            }
            ExprKind::PseudoObjectExpr => {
                self.expr(exprs.pseudo_syntactic(e, &ast.lists), depth, out);
            }
            // clang's `VisitCXXOperatorCallExpr`.
            ExprKind::CXXOperatorCallExpr => {
                use crate::names::OverloadedOperator as O;
                let op = exprs.operator_call_op(e);
                let args: Vec<ExprId> = ast.lists.iter::<ExprId>(exprs.call_args(e)).collect();
                match (op, args.as_slice()) {
                    (O::Call, [obj, rest @ ..]) => {
                        self.expr(*obj, d, out);
                        out.push('(');
                        for (i, &a) in rest.iter().enumerate() {
                            if i > 0 {
                                out.push_str(", ");
                            }
                            self.expr(a, d, out);
                        }
                        out.push(')');
                    }
                    (O::Subscript, [obj, rest @ ..]) => {
                        self.expr(*obj, d, out);
                        out.push('[');
                        for (i, &a) in rest.iter().enumerate() {
                            if i > 0 {
                                out.push_str(", ");
                            }
                            self.expr(a, d, out);
                        }
                        out.push(']');
                    }
                    (O::Arrow, [obj, ..]) => {
                        self.expr(*obj, d, out);
                        out.push_str("->");
                    }
                    (O::PlusPlus | O::MinusMinus, [x, _]) => {
                        self.expr(*x, d, out);
                        out.push_str(op.spelling());
                    }
                    (_, [x]) => {
                        out.push_str(op.spelling());
                        self.expr(*x, d, out);
                    }
                    (_, [l, r, ..]) => {
                        self.expr(*l, d, out);
                        out.push(' ');
                        out.push_str(op.spelling());
                        out.push(' ');
                        self.expr(*r, d, out);
                    }
                    _ => out.push_str(op.spelling()),
                }
            }
            ExprKind::ImplicitCastExpr
            | ExprKind::ConstantExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::CXXStdInitializerListExpr
            | ExprKind::ExprWithCleanups => {
                self.expr(exprs.operand(e), depth, out);
            }
            ExprKind::CXXThisExpr => out.push_str("this"),
            // clang's `VisitCXXTemporaryObjectExpr`: `T(args)`, `T{args}`.
            ExprKind::CXXTemporaryObjectExpr => {
                write_type(&ast.types, self, n.ty(), self.expr_types, out);
                let list = exprs.construct_info(e).list;
                let (open, close) = if list { ('{', '}') } else { ('(', ')') };
                self.expr_list(exprs.construct_args(e), open, close, d, out);
            }
            // clang's `VisitCXXScalarValueInitExpr`: `int()`, the written
            // type with the substituted arguments.
            ExprKind::CXXScalarValueInitExpr => {
                write_type(&ast.types, self, n.ty(), self.expr_types, out);
                out.push_str("()");
            }
            // clang's `VisitCXXConstructExpr`: the arguments (braced for a
            // list-initialization).
            ExprKind::CXXConstructExpr => {
                let info = exprs.construct_info(e);
                let braced = info.list && !info.std_init_list;
                if braced {
                    out.push('{');
                }
                for (i, a) in ast.lists.iter::<ExprId>(exprs.construct_args(e)).enumerate() {
                    if exprs.get(a).kind() == ExprKind::CXXDefaultArgExpr {
                        break;
                    }
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.expr(a, d, out);
                }
                if braced {
                    out.push('}');
                }
            }
            // clang prints the replacement of a substituted parameter.
            ExprKind::SubstNonTypeTemplateParmExpr => {
                self.expr(exprs.subst_replacement(e), depth, out);
            }
            ExprKind::CStyleCastExpr => {
                out.push('(');
                self.written_cast_type(n.ty(), n.value_category(), out);
                out.push(')');
                self.expr(exprs.operand(e), d, out);
            }
            ExprKind::CXXFunctionalCastExpr => {
                write_type(&ast.types, self, n.ty(), self.expr_types, out);
                let sub = exprs.operand(e);
                let braced = exprs.get(sub).kind() == ExprKind::InitListExpr;
                if !braced {
                    out.push('(');
                }
                self.expr(sub, d, out);
                if !braced {
                    out.push(')');
                }
            }
            k @ (ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXDynamicCastExpr
            | ExprKind::BuiltinBitCastExpr) => {
                out.push_str(match k {
                    ExprKind::CXXStaticCastExpr => "static_cast<",
                    ExprKind::CXXConstCastExpr => "const_cast<",
                    ExprKind::CXXReinterpretCastExpr => "reinterpret_cast<",
                    ExprKind::CXXDynamicCastExpr => "dynamic_cast<",
                    _ => "__builtin_bit_cast(",
                });
                self.written_cast_type(n.ty(), n.value_category(), out);
                out.push_str(if k == ExprKind::BuiltinBitCastExpr { ", " } else { ">(" });
                self.expr(exprs.operand(e), d, out);
                out.push(')');
            }
            ExprKind::ImaginaryLiteral => {
                // clang `StmtPrinter::VisitImaginaryLiteral`.
                self.expr(exprs.operand(e), d, out);
                out.push('i');
            }
            ExprKind::ShuffleVectorExpr => {
                // clang `StmtPrinter::VisitShuffleVectorExpr`.
                out.push_str("__builtin_shufflevector(");
                for (i, c) in ast.lists.iter::<ExprId>(exprs.shuffle_subs(e)).enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.expr(c, d, out);
                }
                out.push(')');
            }
            // clang `StmtPrinter::VisitExtVectorElementExpr`: always `.`.
            ExprKind::ExtVectorElementExpr => {
                self.expr(exprs.operand(e), d, out);
                out.push('.');
                out.push_str(ast.symbols().as_str(exprs.ext_vector_accessor(e)));
            }
            ExprKind::ConvertVectorExpr => {
                out.push_str("__builtin_convertvector(");
                self.expr(exprs.operand(e), d, out);
                out.push_str(", ");
                write_type(&ast.types, self, n.ty(), self.expr_types, out);
                out.push(')');
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                let kind = exprs.trait_kind(e);
                out.push_str(kind.spelling());
                if let Some(t) = exprs.trait_type_arg(e) {
                    out.push('(');
                    write_type(&ast.types, self, t, self.expr_types, out);
                    out.push(')');
                } else {
                    // clang StmtPrinter: an expression operand follows a
                    // space for every kind; its own parens (a ParenExpr)
                    // print themselves, so `alignof(g)` reads `alignof (g)`.
                    out.push(' ');
                    self.expr(ExprId::new(n.a()), d, out);
                }
            }
            ExprKind::TypeTraitExpr => {
                out.push_str(exprs.type_trait(e).spelling());
                out.push('(');
                for (i, t) in ast.lists.iter::<QualType>(exprs.type_trait_args(e)).enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write_type(&ast.types, self, t, self.expr_types, out);
                }
                out.push(')');
            }
            ExprKind::ArrayTypeTraitExpr => {
                out.push_str(exprs.type_trait(e).spelling());
                out.push('(');
                write_type(&ast.types, self, exprs.array_trait_type(e), self.expr_types, out);
                if let Some(dim) = exprs.array_trait(e).dim {
                    out.push_str(", ");
                    self.expr(dim, d, out);
                }
                out.push(')');
            }
            ExprKind::ExpressionTraitExpr => {
                out.push_str(exprs.type_trait(e).spelling());
                out.push('(');
                self.expr(ExprId::new(n.a()), d, out);
                out.push(')');
            }
            ExprKind::AtomicExpr => {
                // clang's `StmtPrinter::VisitAtomicExpr`: the builtin's
                // name and the operands in source order.
                use crate::decl::builtin::atomic;
                let op = exprs.atomic_op(e);
                out.push_str(atomic::name(op));
                out.push('(');
                let subs: Vec<ExprId> = ast.lists.iter::<ExprId>(exprs.atomic_subs(e)).collect();
                let order = atomic::form(op).map_or(&[][..], atomic::Form::arg_to_sub);
                for (i, &k) in order.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    if let Some(&c) = subs.get(k) {
                        self.expr(c, d, out);
                    }
                }
                out.push(')');
            }
            ExprKind::VAArgExpr => {
                let (list, written, _) = exprs.va_arg(e);
                out.push_str("__builtin_va_arg(");
                self.expr(list, d, out);
                out.push_str(", ");
                write_type(&ast.types, self, written, self.expr_types, out);
                out.push(')');
            }
            ExprKind::CXXNoexceptExpr => {
                out.push_str("noexcept(");
                self.expr(exprs.noexcept_operand(e), d, out);
                out.push(')');
            }
            ExprKind::InitListExpr => {
                self.expr_list(exprs.init_list_inits(e), '{', '}', d, out);
            }
            ExprKind::DesignatedInitExpr => {
                for x in exprs.designators(e) {
                    match *x {
                        crate::expr::Designator::Field { name, .. } => {
                            out.push('.');
                            out.push_str(ast.symbols().as_str(name));
                        }
                        crate::expr::Designator::Index { index, .. } => {
                            out.push('[');
                            self.expr(index, d, out);
                            out.push(']');
                        }
                        crate::expr::Designator::Range { first, last, .. } => {
                            out.push('[');
                            self.expr(first, d, out);
                            out.push_str(" ... ");
                            self.expr(last, d, out);
                            out.push(']');
                        }
                    }
                }
                let init = exprs.designated_init(e);
                if exprs.get(init).kind() != ExprKind::InitListExpr {
                    out.push_str(" = ");
                }
                self.expr(init, d, out);
            }
            ExprKind::PackExpansionExpr => {
                self.expr(exprs.operand(e), d, out);
                out.push_str("...");
            }
            ExprKind::PackIndexingExpr => {
                self.expr(exprs.operand(e), d, out);
                out.push_str("...[");
                if let Some(i) = ExprId::from_raw(n.b()) {
                    self.expr(i, d, out);
                }
                out.push(']');
            }
            ExprKind::CXXUnresolvedConstructExpr => {
                write_type(&ast.types, self, n.ty(), self.expr_types, out);
                let (open, close) = if n.aux() != 0 { ('{', '}') } else { ('(', ')') };
                let args = ListId::from_raw(n.a());
                // The braced form's one argument is the list itself.
                let list = args
                    .map(|l| ast.lists.iter::<ExprId>(l).collect::<Vec<_>>())
                    .filter(|v| n.aux() != 0 && v.len() == 1)
                    .map(|v| v[0])
                    .filter(|&x| exprs.get(x).kind() == ExprKind::InitListExpr);
                if let Some(x) = list {
                    self.expr(x, d, out);
                } else if let Some(l) = args {
                    self.expr_list(l, open, close, d, out);
                } else {
                    out.push(open);
                    out.push(close);
                }
            }
            ExprKind::SizeOfPackExpr => {
                out.push_str("sizeof...(");
                if let Some(p) = DeclId::from_raw(n.a()) {
                    self.write_name(p, out);
                }
                out.push(')');
            }
            // clang's `VisitRequiresExpr`.
            ExprKind::RequiresExpr => {
                let Some(rq) = ast.requires_of(e) else {
                    out.push_str("...");
                    return;
                };
                out.push_str("requires ");
                let mut parms = ast.lists.iter::<DeclId>(rq.params).peekable();
                if parms.peek().is_some() {
                    out.push('(');
                    for (i, p) in parms.enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        let mut name = String::new();
                        self.write_name(p, &mut name);
                        match ast.decls.get(p).ty() {
                            Some(t) => crate::types::write_declarator(
                                &ast.types,
                                self,
                                t,
                                self.expr_types,
                                &name,
                                out,
                            ),
                            None => out.push_str(&name),
                        }
                    }
                    out.push_str(") ");
                }
                out.push_str("{ ");
                for r in ast.templates.requirements(rq) {
                    match *r {
                        crate::templates::Requirement::Simple(x) => self.expr(x, d, out),
                        crate::templates::Requirement::Type(t, _) => {
                            write_type(&ast.types, self, t, self.expr_types, out);
                        }
                        crate::templates::Requirement::Compound { expr, noexcept, constraint } => {
                            out.push_str("{ ");
                            self.expr(expr, d, out);
                            out.push_str(" }");
                            if noexcept.is_some() {
                                out.push_str(" noexcept");
                            }
                            if let Some(c) = constraint {
                                out.push_str(" -> ");
                                self.type_constraint(c, out);
                            }
                        }
                        crate::templates::Requirement::Nested(x) => {
                            out.push_str("requires ");
                            self.expr(x, d, out);
                        }
                    }
                    out.push_str("; ");
                }
                out.push('}');
            }
            ExprKind::CXXFoldExpr => {
                let op = crate::expr::BinaryOp::from_u8(n.aux()).map_or("?", |o| o.spelling());
                out.push('(');
                if let Some(l) = ExprId::from_raw(n.a()) {
                    self.expr(l, d, out);
                    let _ = write!(out, " {op} ");
                }
                out.push_str("...");
                if let Some(r) = ExprId::from_raw(n.b()) {
                    let _ = write!(out, " {op} ");
                    self.expr(r, d, out);
                }
                out.push(')');
            }
            // clang's `VisitDependentScopeDeclRefExpr` /
            // `VisitUnresolvedLookupExpr`: `N::name<args>`.
            ExprKind::DependentScopeDeclRefExpr => {
                write_nns(&ast.types, self, crate::ids::NnsId::new(n.a()), out);
                if ast.templates.template_kw.contains(e) {
                    out.push_str("template ");
                }
                self.dependent_name(e, ast.dependent_scope_ref_name(e), out);
            }
            ExprKind::UnresolvedLookupExpr => {
                if let Some(&nns) = ast.templates.expr_nns.get(e) {
                    write_nns(&ast.types, self, nns, out);
                }
                if ast.templates.template_kw.contains(e) {
                    out.push_str("template ");
                }
                self.dependent_name(e, ast.unresolved_name(e), out);
            }
            // clang's `VisitCXXDependentScopeMemberExpr`.
            ExprKind::CXXDependentScopeMemberExpr => {
                if let Some(b) = ExprId::from_raw(n.a()) {
                    self.expr(b, d, out);
                    out.push_str(if n.aux() & 1 != 0 { "->" } else { "." });
                }
                if n.aux() & 2 != 0 {
                    out.push_str("template ");
                }
                if let Some(&nns) = ast.templates.expr_nns.get(e) {
                    write_nns(&ast.types, self, nns, out);
                }
                self.dependent_name(e, ast.dependent_member_name(e), out);
            }
            // clang's `VisitCXXPseudoDestructorExpr`: `p->~T`.
            ExprKind::CXXPseudoDestructorExpr => {
                let (b, arrow, t) = exprs.pseudo_destructor(e);
                self.expr(b, d, out);
                out.push_str(if arrow { "->~" } else { ".~" });
                match ast.templates.pseudo_dtor_name.get(e) {
                    Some(&n) => out.push_str(ast.symbols().as_str(n)),
                    None => write_type(&ast.types, self, t, PrintOptions::SUGARED, out),
                }
            }
            // clang's `ConceptReference::print`: the qualifier as written,
            // the concept's name and its converted arguments.
            ExprKind::ConceptSpecializationExpr if DeclId::from_raw(n.a()).is_some() => {
                if let Some(&nns) = ast.templates.expr_nns.get(e) {
                    write_nns(&ast.types, self, nns, out);
                }
                self.write_name(DeclId::new(n.a()), out);
                out.push('<');
                if let Some(args) = crate::ids::ArgListId::from_raw(n.b()) {
                    for (i, a) in ast.types.args(args).iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        match a.view() {
                            ArgView::Type(t) => {
                                write_type(&ast.types, self, t, self.expr_types, out);
                            }
                            _ => crate::types::write_template_arg(&ast.types, self, *a, out),
                        }
                    }
                }
                out.push('>');
            }
            _ => out.push_str("..."),
        }
    }

    /// The name and explicit template arguments of dependent name
    /// expression `e`.
    fn dependent_name(&self, e: ExprId, name: DeclName, out: &mut String) {
        self.write_decl_name_value(name, out);
        if let Some(&l) = self.ast.templates.expr_targs.get(e) {
            write_arg_list(&self.ast.types, self, l, out);
        }
    }

    fn expr_list(&self, list: ListId, open: char, close: char, depth: u32, out: &mut String) {
        out.push(open);
        for (i, a) in self.ast.lists.iter::<ExprId>(list).enumerate() {
            // clang stops at the first defaulted argument.
            if self.ast.exprs.get(a).kind() == ExprKind::CXXDefaultArgExpr {
                break;
            }
            if i > 0 {
                out.push_str(", ");
            }
            self.expr(a, depth, out);
        }
        out.push(close);
    }

    /// Appends constant `v` in the dump's `value=` syntax (spec/ast.md
    /// §9.3).
    pub fn write_dump_value(&self, v: ConstValueId, out: &mut String) {
        self.ast.consts.print(v, &self.ast.strings, self, &ValueOptions::DEFAULT, out);
    }
}

impl AstNames<'_> {
    /// An explicit cast's type as written (clang's `getTypeAsWritten`):
    /// the expression's type `t` plus the reference its value category
    /// implies, an lvalue cast naming `T &` and an xvalue cast `T &&`.
    /// The interned reference node is preferred, so `int (&)[2]` prints
    /// with its declarator parentheses (WP-145).
    fn written_cast_type(&self, t: QualType, vk: ValueCategory, out: &mut String) {
        let types = &self.ast.types;
        let rvalue = match vk {
            ValueCategory::PRValue => {
                return write_type(types, self, t, self.expr_types, out);
            }
            ValueCategory::LValue => false,
            ValueCategory::XValue => true,
        };
        if let Some(r) = types.find_reference_to(t, rvalue) {
            write_type(types, self, r, self.expr_types, out);
        } else {
            write_type(types, self, t, self.expr_types, out);
            out.push_str(if rvalue { " &&" } else { " &" });
        }
    }

    /// A return-type-requirement's type-constraint as written: the
    /// immediately-declared constraint `C<decltype((e)), args...>` printed
    /// as `C<args...>` (clang's `TypeConstraint::print`).
    fn type_constraint(&self, c: ExprId, out: &mut String) {
        let x = self.ast.exprs.get(c);
        let Some(concept) =
            DeclId::from_raw(x.a()).filter(|_| x.kind() == ExprKind::ConceptSpecializationExpr)
        else {
            out.push_str("...");
            return;
        };
        self.write_name(concept, out);
        let Some(args) = crate::ids::ArgListId::from_raw(x.b()) else { return };
        let all = self.ast.types.args(args);
        if all.len() > 1 {
            out.push('<');
            for (i, a) in all.iter().skip(1).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                crate::types::write_template_arg(&self.ast.types, self, *a, out);
            }
            out.push('>');
        }
    }

    /// [`TypeNames::invented_parm`]: an implicit type template parameter
    /// and its type-constraint's concept and arguments.
    #[allow(clippy::option_option, reason = "the TypeNames::invented_parm contract")]
    fn invented_parm_of(&self, decl: DeclId) -> Option<Option<(DeclId, crate::ids::ArgListId)>> {
        let n = self.ast.decls.get(decl);
        if n.kind() != DeclKind::TemplateTypeParm || !n.flags().contains(DeclFlags::IMPLICIT) {
            return None;
        }
        let c = self.ast.template_parm(decl)?.constraint;
        Some(c.and_then(|e| {
            let mut x = self.ast.exprs.get(e);
            // A constrained pack's constraint is the fold `(C<T> && ...)`.
            if x.kind() == ExprKind::CXXFoldExpr {
                x = self.ast.exprs.get(ExprId::from_raw(x.a())?);
            }
            if x.kind() != ExprKind::ConceptSpecializationExpr {
                return None;
            }
            Some((DeclId::from_raw(x.a())?, crate::ids::ArgListId::from_raw(x.b())?))
        }))
    }
}

impl TypeNames for AstNames<'_> {
    fn implicit_typedef(&self, decl: DeclId) -> bool {
        self.ast.decls.get(decl).flags().contains(DeclFlags::IMPLICIT)
    }

    fn specialization_args(&self, decl: DeclId) -> Option<crate::ids::ArgListId> {
        self.ast.spec_print_args(decl)
    }

    fn preferred_name(&self, decl: DeclId) -> Option<QualType> {
        self.ast.preferred_name_of(decl)
    }

    fn invented_parm(&self, decl: DeclId) -> Option<Option<(DeclId, crate::ids::ArgListId)>> {
        self.invented_parm_of(decl)
    }

    fn converted_args(&self, decl: DeclId) -> Option<crate::ids::ArgListId> {
        if self.ast.decls.get(decl).kind() != DeclKind::CXXRecord {
            return None;
        }
        self.ast.spec_info(decl).map(|(_, s)| s.args)
    }

    fn write_converted_scope(&self, decl: DeclId, out: &mut String) -> bool {
        self.append_converted_scope(self.ast.decls.get(decl).ctx(), 0, out);
        true
    }

    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        let n = self.ast.decls.get(decl);
        let tag = matches!(
            n.kind(),
            DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization
                | DeclKind::Enum
        );
        if tag && self.is_unnamed(decl) {
            if let Some(td) = self.ast.decls.typedef_for_anon(decl) {
                if scoped {
                    self.append_scope(self.ast.decls.get(td).ctx(), 0, out);
                }
                self.write_name(td, out);
            } else {
                // An anonymous struct/union cannot be written, so its type
                // only appears bare (the implicit member), which clang
                // prints scoped.
                let anonymous = self
                    .ast
                    .decls
                    .record(decl)
                    .is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS));
                if scoped || anonymous {
                    self.append_scope(n.ctx(), 0, out);
                }
                self.write_unnamed_tag(decl, out);
            }
            return;
        }
        if scoped {
            self.append_scope(n.ctx(), 0, out);
        }
        self.write_name(decl, out);
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        self.expr(expr, 0, out);
    }

    fn write_value(&self, _ty: QualType, value: ConstValueId, out: &mut String) {
        // A converted argument of a known parameter (clang's
        // `IncludeType` is false unless the parameter is `auto`; WP-071).
        let opts = ValueOptions {
            include_type: false,
            ..ValueOptions::with_style(PrintStyle::TemplateArg)
        };
        self.ast.consts.print(value, &self.ast.strings, self, &opts, out);
    }

    fn write_value_typed(&self, _ty: QualType, value: ConstValueId, out: &mut String) {
        let opts = ValueOptions {
            include_type: true,
            ..ValueOptions::with_style(PrintStyle::TemplateArg)
        };
        self.ast.consts.print(value, &self.ast.strings, self, &opts, out);
    }

    fn arg_includes_type(&self, template: DeclId, index: usize) -> bool {
        self.ast.nttp_has_deduced_type(template, index)
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.ast.symbols().as_str(sym));
    }

    fn write_template_param(&self, associated: DeclId, index: u32, out: &mut String) {
        self.ast.write_replaced_param(associated, index, out);
    }

    fn is_anonymous_namespace(&self, ns: DeclId) -> bool {
        self.ast
            .decls
            .namespace(ns)
            .is_some_and(|n| n.flags & crate::decl::NamespaceData::ANONYMOUS != 0)
            || self.is_unnamed(ns)
    }
}

impl PrintEnv for AstNames<'_> {
    fn shape(&self, ty: QualType) -> Shape {
        self.shape_at(ty, 0)
    }

    fn decl_name(&self, decl: DeclId, out: &mut String) {
        self.write_qualified_name(decl, out);
    }

    fn decl_type(&self, decl: DeclId) -> Option<QualType> {
        self.ast.decls.get(decl).ty()
    }

    fn same_unqualified(&self, a: QualType, b: QualType) -> bool {
        // Array element cv too: `&obj.arr` of a const `obj` points to a
        // `const int[2]` while the member is an `int[2]`.
        self.ast.types.same_unqualified(a, b)
    }

    fn type_name(&self, ty: QualType, out: &mut String) {
        write_type(&self.ast.types, self, ty, PrintOptions::SUGARED, out);
    }

    fn arg_type_name(&self, ty: QualType, out: &mut String) {
        write_type(&self.ast.types, self, ty, PrintOptions::CANONICAL, out);
    }

    fn enumerator(&self, ty: QualType, bits: &IntBits<'_>, out: &mut String) -> bool {
        let Some(e) = self.ast.types.as_enum(ty) else { return false };
        let e = self.ast.definition(e).unwrap_or(e);
        for m in self.ast.members(e) {
            let Some(v) = self.ast.decls.enumerator(m).and_then(|d| d.value) else { continue };
            // An enumerator of a `bool`-based enumeration holds a `bool`.
            let same = match self.ast.consts.scalar(v) {
                Some(Scalar::Int { bits: b, .. }) => b == *bits,
                Some(Scalar::Bool { value, .. }) => bits.to_u128() == Some(u128::from(value)),
                _ => false,
            };
            if same {
                self.write_qualified_name(m, out);
                return true;
            }
        }
        false
    }

    /// A reflection in its stable form (spec/reflection-repr.md §5):
    /// `^^int`, `^^ns::f`, `^^::`, `^^(value of type int)`.
    fn reflection(&self, id: crate::ids::ReflectionId, out: &mut String) {
        use crate::reflect::Reflectee;
        use std::fmt::Write;
        let r = self.ast.reflections.get(id);
        if r == Reflectee::Null {
            out.push_str("std::meta::info()");
            return;
        }
        out.push_str("^^");
        match r {
            Reflectee::Null => {}
            Reflectee::Type(t) => self.type_name(t, out),
            Reflectee::Namespace(d)
                if self.ast.decls.get(d).kind() == DeclKind::TranslationUnit =>
            {
                out.push_str("::");
            }
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.write_qualified_name(d, out);
            }
            Reflectee::Template(n) => match self.ast.types.template_decl(n) {
                Some(d) => self.write_qualified_name(d, out),
                None => out.push_str("(template)"),
            },
            Reflectee::Value(v) | Reflectee::Object(v) => {
                let object = matches!(r, Reflectee::Object(_));
                let mut t = self.ast.consts.ty(v);
                if object {
                    t = self.ast.types.pointee(t).unwrap_or(t);
                }
                out.push_str(if object { "(object of type " } else { "(value of type " });
                self.type_name(t, out);
                out.push(')');
            }
            Reflectee::BaseSpec(d, i) => {
                let _ = write!(out, "(base #{i} of ");
                self.write_qualified_name(d, out);
                out.push(')');
            }
            Reflectee::DataMemberSpec(m) => {
                let _ = write!(out, "(data member spec #{})", m.index());
            }
            Reflectee::Annotation(a) => {
                let _ = write!(out, "(annotation #{})", a.index());
            }
            Reflectee::FnParam(f, i) => {
                let _ = write!(out, "(parameter #{i} of ");
                self.write_qualified_name(f, out);
                out.push(')');
            }
        }
    }
}

impl TypeNames for Ast {
    fn implicit_typedef(&self, decl: DeclId) -> bool {
        self.decls.get(decl).flags().contains(DeclFlags::IMPLICIT)
    }

    fn specialization_args(&self, decl: DeclId) -> Option<crate::ids::ArgListId> {
        self.spec_print_args(decl)
    }

    fn preferred_name(&self, decl: DeclId) -> Option<QualType> {
        self.preferred_name_of(decl)
    }

    fn invented_parm(&self, decl: DeclId) -> Option<Option<(DeclId, crate::ids::ArgListId)>> {
        AstNames::new(self).invented_parm_of(decl)
    }

    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        AstNames::new(self).write_decl_name(decl, scoped, out);
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        AstNames::new(self).write_expr(expr, out);
    }

    fn write_value(&self, ty: QualType, value: ConstValueId, out: &mut String) {
        AstNames::new(self).write_value(ty, value, out);
    }

    fn write_value_typed(&self, ty: QualType, value: ConstValueId, out: &mut String) {
        AstNames::new(self).write_value_typed(ty, value, out);
    }

    fn arg_includes_type(&self, template: DeclId, index: usize) -> bool {
        self.nttp_has_deduced_type(template, index)
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.symbols().as_str(sym));
    }

    fn write_template_param(&self, associated: DeclId, index: u32, out: &mut String) {
        self.write_replaced_param(associated, index, out);
    }

    fn is_anonymous_namespace(&self, ns: DeclId) -> bool {
        AstNames::new(self).is_anonymous_namespace(ns)
    }
}

impl PrintEnv for Ast {
    fn shape(&self, ty: QualType) -> Shape {
        AstNames::new(self).shape(ty)
    }

    fn decl_name(&self, decl: DeclId, out: &mut String) {
        AstNames::new(self).write_qualified_name(decl, out);
    }

    fn decl_type(&self, decl: DeclId) -> Option<QualType> {
        self.decls.get(decl).ty()
    }

    fn same_unqualified(&self, a: QualType, b: QualType) -> bool {
        AstNames::new(self).same_unqualified(a, b)
    }

    fn type_name(&self, ty: QualType, out: &mut String) {
        AstNames::new(self).type_name(ty, out);
    }

    fn arg_type_name(&self, ty: QualType, out: &mut String) {
        AstNames::new(self).arg_type_name(ty, out);
    }

    fn enumerator(&self, ty: QualType, bits: &IntBits<'_>, out: &mut String) -> bool {
        AstNames::new(self).enumerator(ty, bits, out)
    }

    fn reflection(&self, id: crate::ids::ReflectionId, out: &mut String) {
        AstNames::new(self).reflection(id, out);
    }
}

/// True if `f` is a decl that carries `DeclFlags::IMPLICIT`.
pub(super) fn is_implicit(ast: &Ast, d: DeclId) -> bool {
    ast.decls.get(d).has(DeclFlags::IMPLICIT)
}
