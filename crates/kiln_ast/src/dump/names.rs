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
use crate::expr::{CharKind, ExprFlags, ExprKind, TraitKind};
use crate::ids::{ConstValueId, DeclId, ExprId, ListId, QualType, RawId, TypeId};
use crate::names::{DeclName, DeclNameView};
use crate::strings::{StringEncoding, push_unit};
use crate::types::{
    FloatFormat, PrintOptions, TypeNames, TypeView, write_arg_list, write_nns, write_type,
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
}

impl std::fmt::Debug for AstNames<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AstNames").field("sources", &self.sources.is_some()).finish_non_exhaustive()
    }
}

impl<'a> AstNames<'a> {
    /// Names over `ast` without locations.
    pub fn new(ast: &'a Ast) -> Self {
        AstNames { ast, sources: None }
    }

    /// Names over `ast` printing unnamed tags with their location.
    pub fn with_sources(ast: &'a Ast, sources: &'a SourceManager) -> Self {
        AstNames { ast, sources: Some(sources) }
    }

    /// Appends the unqualified name of `d` as the dump prints it
    /// (clang's `NamedDecl::printName`): identifiers, `operator+`, the
    /// class name for constructors, `~S`, `operator int`,
    /// `operator""_x`, `<deduction guide for S>`. Unnamed declarations
    /// append nothing.
    pub fn write_name(&self, d: DeclId, out: &mut String) {
        self.write_decl_name_value(self.ast.decls.get(d).name(), out);
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
        let n = self.ast.decls.get(d);
        if let Some(ctx) = n.ctx()
            && !self.ast.decls.get(ctx).kind().is_function()
        {
            self.write_scope_chain(ctx, 0, out);
        }
        self.write_component(d, out);
    }

    /// Appends `ctx` and its parents, each followed by `::`.
    fn write_scope_chain(&self, ctx: DeclId, depth: u32, out: &mut String) {
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
            self.write_scope_chain(parent, depth + 1, out);
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

    /// The value of a constant integer expression (a bit-field width).
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
            4 | 10 | 18 => (true, IntStyle::Cast),
            5 | 11 | 19 => (false, IntStyle::Cast),
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
            22 => FloatKind::Single,
            23 => FloatKind::Double,
            24 => match self.ast.types.target().long_double {
                FloatFormat::IeeeHalf => FloatKind::Half,
                FloatFormat::BFloat => FloatKind::BFloat16,
                FloatFormat::IeeeSingle => FloatKind::Single,
                FloatFormat::IeeeDouble => FloatKind::Double,
                FloatFormat::X87Extended => FloatKind::X87Extended,
                FloatFormat::IeeeQuad => FloatKind::Quad,
                FloatFormat::PpcDoubleDouble => return None,
            },
            25 => FloatKind::Quad,
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
            ExprKind::FloatingLiteral => {
                if let Some(c) = exprs.literal_const(e) {
                    self.write_dump_value(c, out);
                }
            }
            ExprKind::CharacterLiteral => {
                out.push_str(match exprs.char_kind(e) {
                    CharKind::Ascii => "",
                    CharKind::Wide => "L",
                    CharKind::Utf8 => "u8",
                    CharKind::Utf16 => "u",
                    CharKind::Utf32 => "U",
                });
                out.push('\'');
                push_unit(exprs.char_value(e), '\'', out);
                out.push('\'');
            }
            ExprKind::StringLiteral => ast.strings.write_literal(exprs.string_lit(e), out),
            ExprKind::CXXBoolLiteralExpr => {
                out.push_str(if exprs.bool_value(e) { "true" } else { "false" });
            }
            ExprKind::CXXNullPtrLiteralExpr => out.push_str("nullptr"),
            ExprKind::DeclRefExpr => self.write_name(exprs.decl(e), out),
            ExprKind::MemberExpr => {
                self.expr(exprs.operand(e), d, out);
                out.push_str(if n.has(ExprFlags::ARROW) { "->" } else { "." });
                self.write_name(exprs.decl(e), out);
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
            ExprKind::CallExpr => {
                self.expr(exprs.callee(e), d, out);
                self.expr_list(exprs.call_args(e), '(', ')', d, out);
            }
            ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                self.expr(exprs.operand(e), depth, out);
            }
            ExprKind::CStyleCastExpr => {
                out.push('(');
                write_type(&ast.types, self, n.ty(), PrintOptions::SUGARED, out);
                out.push(')');
                self.expr(exprs.operand(e), d, out);
            }
            ExprKind::CXXFunctionalCastExpr => {
                write_type(&ast.types, self, n.ty(), PrintOptions::SUGARED, out);
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
                write_type(&ast.types, self, n.ty(), PrintOptions::SUGARED, out);
                out.push_str(if k == ExprKind::BuiltinBitCastExpr { ", " } else { ">(" });
                self.expr(exprs.operand(e), d, out);
                out.push(')');
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                let kind = exprs.trait_kind(e);
                out.push_str(kind.spelling());
                if let Some(t) = exprs.trait_type_arg(e) {
                    out.push('(');
                    write_type(&ast.types, self, t, PrintOptions::SUGARED, out);
                    out.push(')');
                } else {
                    let paren = kind != TraitKind::SizeOf;
                    out.push(if paren { '(' } else { ' ' });
                    self.expr(ExprId::new(n.a()), d, out);
                    if paren {
                        out.push(')');
                    }
                }
            }
            ExprKind::CXXNoexceptExpr => {
                out.push_str("noexcept(");
                self.expr(exprs.noexcept_operand(e), d, out);
                out.push(')');
            }
            ExprKind::InitListExpr => {
                self.expr_list(exprs.init_list_inits(e), '{', '}', d, out);
            }
            ExprKind::PackExpansionExpr => {
                self.expr(exprs.operand(e), d, out);
                out.push_str("...");
            }
            ExprKind::CXXUnresolvedConstructExpr => {
                write_type(&ast.types, self, n.ty(), PrintOptions::SUGARED, out);
                let (open, close) = if n.aux() != 0 { ('{', '}') } else { ('(', ')') };
                if let Some(l) = ListId::from_raw(n.a()) {
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
                self.dependent_name(e, Symbol::from_index(n.b()), out);
            }
            ExprKind::UnresolvedLookupExpr => {
                if let Some(&nns) = ast.templates.expr_nns.get(e) {
                    write_nns(&ast.types, self, nns, out);
                }
                self.dependent_name(e, Symbol::from_index(n.a()), out);
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
                self.dependent_name(e, Symbol::from_index(n.b()), out);
            }
            _ => out.push_str("..."),
        }
    }

    /// The name and explicit template arguments of dependent name
    /// expression `e`.
    fn dependent_name(&self, e: ExprId, name: Symbol, out: &mut String) {
        self.write_symbol(name, out);
        if let Some(&l) = self.ast.templates.expr_targs.get(e) {
            write_arg_list(&self.ast.types, self, l, out);
        }
    }

    fn expr_list(&self, list: ListId, open: char, close: char, depth: u32, out: &mut String) {
        out.push(open);
        for (i, a) in self.ast.lists.iter::<ExprId>(list).enumerate() {
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
    fn invented_parm(&self, decl: DeclId) -> Option<Option<(DeclId, crate::ids::ArgListId)>> {
        self.invented_parm_of(decl)
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
        let opts = ValueOptions::with_style(PrintStyle::TemplateArg);
        self.ast.consts.print(value, &self.ast.strings, self, &opts, out);
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.ast.symbols().as_str(sym));
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

    fn type_name(&self, ty: QualType, out: &mut String) {
        write_type(&self.ast.types, self, ty, PrintOptions::SUGARED, out);
    }

    fn enumerator(&self, ty: QualType, bits: &IntBits<'_>, out: &mut String) -> bool {
        let Some(e) = self.ast.types.as_enum(ty) else { return false };
        let e = self.ast.definition(e).unwrap_or(e);
        for m in self.ast.members(e) {
            let Some(v) = self.ast.decls.enumerator(m).and_then(|d| d.value) else { continue };
            if let Some(Scalar::Int { bits: b, .. }) = self.ast.consts.scalar(v)
                && b == *bits
            {
                self.write_qualified_name(m, out);
                return true;
            }
        }
        false
    }
}

impl TypeNames for Ast {
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

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.symbols().as_str(sym));
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

    fn type_name(&self, ty: QualType, out: &mut String) {
        AstNames::new(self).type_name(ty, out);
    }

    fn enumerator(&self, ty: QualType, bits: &IntBits<'_>, out: &mut String) -> bool {
        AstNames::new(self).enumerator(ty, bits, out)
    }
}

/// True if `f` is a decl that carries `DeclFlags::IMPLICIT`.
pub(super) fn is_implicit(ast: &Ast, d: DeclId) -> bool {
    ast.decls.get(d).has(DeclFlags::IMPLICIT)
}
