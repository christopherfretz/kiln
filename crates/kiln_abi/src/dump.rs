//! The record-layout dump (spec/abi.md §6): the text clang 19 prints for
//! `-Xclang -fdump-record-layouts`, byte for byte, so layouts can be
//! compared with clang's directly.
//!
//! clang prints these types with a default-constructed `PrintingPolicy`,
//! i.e. the *C* policy: `_Bool`, `nullptr_t`, `(void)` for an empty
//! parameter list, and the tag keyword on bare class types (the dumped
//! class itself and the class of a member pointer). `CPrinter` is a
//! port of clang's `TypePrinter` for the M2 type kinds with those
//! differences; any other kind is printed by
//! [`kiln_ast::types::write_type`].

use std::fmt::Write as _;

use kiln_ast::decl::{DeclKind, NamespaceData};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{
    BuiltinType, PrintOptions, Quals, TypeNames, TypeView, VectorKind, write_type,
};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, TypeId};
use kiln_source::symbol::Symbol;

use crate::layout::{Layouter, props};
use crate::target::Target;

/// Nesting limit of the type printer (deeper types print `...`).
const MAX_DEPTH: u32 = 256;

/// The `-fdump-record-layouts` block of class `record`, including the
/// leading `\n*** Dumping AST Record Layout\n` line clang emits before
/// each class.
pub fn dump_record_layout(ast: &Ast, record: DeclId, target: &Target) -> String {
    let mut lay = Layouter::new(ast, target);
    let mut out = String::new();
    write_record_layout(&mut lay, record, &mut out);
    out
}

/// One pending piece of a dump.
enum Item {
    /// A class at an absolute byte offset, indent level and field name.
    Record { d: DeclId, off: u64, indent: usize, desc: Option<String> },
    /// Finished text.
    Line(String),
}

/// Appends the dump block of `record` to `out`, reusing `lay`'s cache.
pub fn write_record_layout(lay: &mut Layouter<'_>, record: DeclId, out: &mut String) {
    out.push_str("\n*** Dumping AST Record Layout\n");
    let ast = lay.ast();
    let first = ast.first(record);
    // Explicit stack of frames still to print; nested classes are
    // printed in place (pre-order).
    let mut stack = vec![Item::Record { d: first, off: 0, indent: 0, desc: None }];
    while let Some(item) = stack.pop() {
        let (d, off, indent, desc) = match item {
            Item::Line(s) => {
                out.push_str(&s);
                continue;
            }
            Item::Record { d, off, indent, desc } => (d, off, indent, desc),
        };
        let layout = lay.record(d).clone();
        let empty = lay.is_empty(d);
        print_offset(out, off, indent);
        write_tag_type(ast, d, out);
        if let Some(desc) = &desc {
            out.push(' ');
            out.push_str(desc);
        }
        if empty {
            out.push_str(" (empty)");
        }
        out.push('\n');
        let inner = indent + 1;
        let Some(def) = ast.definition(d) else { continue };
        let fields = props::fields(ast, def);
        // Frames are pushed in reverse so they pop in declaration order.
        let mut frames = Vec::with_capacity(fields.len());
        for (i, f) in fields.iter().enumerate() {
            let bits = layout.fields.get(i).copied().unwrap_or(0);
            let foff = off + bits / 8;
            if let Some(r) = record_of(ast, f.ty) {
                let name = field_name(ast, f.decl);
                frames.push(Item::Record { d: r, off: foff, indent: inner, desc: Some(name) });
                continue;
            }
            let mut line = String::new();
            if let Some(w) = f.bit_width {
                let begin = bits % 8;
                let mut s = format!("{foff}:");
                if w == 0 {
                    s.push('-');
                } else {
                    let _ = write!(s, "{begin}-{}", begin + w - 1);
                }
                let _ = write!(line, "{s:>10} | ");
                push_indent(&mut line, inner);
            } else {
                print_offset(&mut line, foff, inner);
            }
            CPrinter::new(ast).print_into(f.ty, &mut line);
            line.push(' ');
            line.push_str(&field_name(ast, f.decl));
            line.push('\n');
            frames.push(Item::Line(line));
        }
        if desc.is_none() {
            let mut footer = String::new();
            let _ = write!(
                footer,
                "           | [sizeof={}, dsize={}, align={},\n           |  nvsize={}, nvalign={}]\n",
                layout.size, layout.data_size, layout.align, layout.nv_size, layout.nv_align
            );
            frames.push(Item::Line(footer));
        }
        stack.extend(frames.into_iter().rev());
    }
}

fn print_offset(out: &mut String, off: u64, indent: usize) {
    let _ = write!(out, "{off:>10} | ");
    push_indent(out, indent);
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

/// clang `getAs<RecordType>()`: the class a field's type designates
/// through sugar and cv-qualifiers (not arrays or `_Atomic`).
fn record_of(ast: &Ast, q: QualType) -> Option<DeclId> {
    match ast.types.canonical_view(q) {
        TypeView::Record(d) => Some(d),
        _ => None,
    }
}

fn field_name(ast: &Ast, d: DeclId) -> String {
    let name = ast.decls.get(d).name();
    ast.names.simple_spelling(name, ast.symbols()).unwrap_or_default()
}

/// A bare class type in the C policy: `struct ns::S`.
fn write_tag_type(ast: &Ast, d: DeclId, out: &mut String) {
    let tag = ast.decls.record(d).map_or("struct", |r| r.tag.keyword());
    out.push_str(tag);
    out.push(' ');
    write_decl_name(ast, d, true, out);
}

/// Writes the name of `d`, preceded by its enclosing namespaces and
/// classes when `scoped` (clang `AppendScope`; linkage specifications
/// and inline namespaces are transparent).
pub(crate) fn write_decl_name(ast: &Ast, d: DeclId, scoped: bool, out: &mut String) {
    if scoped {
        let mut chain = Vec::new();
        let mut c = ast.decls.get(d).ctx();
        let mut guard = 0;
        while let Some(s) = c {
            guard += 1;
            if guard > 4096 {
                break;
            }
            match ast.decls.get(s).kind() {
                DeclKind::Namespace => {
                    let flags = ast.decls.namespace(s).map_or(0, |n| n.flags);
                    if flags & NamespaceData::INLINE == 0 {
                        chain.push(s);
                    }
                }
                DeclKind::CXXRecord | DeclKind::Enum => chain.push(s),
                _ => {}
            }
            c = ast.decls.get(s).ctx();
        }
        for s in chain.into_iter().rev() {
            write_simple_name(ast, s, out);
            out.push_str("::");
        }
    }
    write_simple_name(ast, d, out);
}

fn write_simple_name(ast: &Ast, d: DeclId, out: &mut String) {
    let node = ast.decls.get(d);
    let name = node.name();
    if name.is_empty() {
        match node.kind() {
            DeclKind::Namespace => out.push_str("(anonymous namespace)"),
            DeclKind::CXXRecord => {
                let tag = ast.decls.record(d).map_or("struct", |r| r.tag.keyword());
                out.push_str("(unnamed ");
                out.push_str(tag);
                out.push(')');
            }
            DeclKind::Enum => out.push_str("(unnamed enum)"),
            _ => {}
        }
        return;
    }
    out.push_str(&ast.names.simple_spelling(name, ast.symbols()).unwrap_or_default());
}

/// [`TypeNames`] over an [`Ast`] for the kinds [`CPrinter`] delegates.
#[derive(Clone, Copy, Debug)]
struct AbiNames<'a> {
    ast: &'a Ast,
}

impl TypeNames for AbiNames<'_> {
    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        write_decl_name(self.ast, decl, scoped, out);
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        write_expr(self.ast, expr, out);
    }

    fn write_value(&self, _ty: QualType, _value: ConstValueId, out: &mut String) {
        out.push_str("<value>");
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.ast.symbols().as_str(sym));
    }
}

/// The expressions that can appear in an M2 field type (`decltype`
/// operands, array bounds of dependent types).
fn write_expr(ast: &Ast, e: ExprId, out: &mut String) {
    match ast.exprs.get(e).kind() {
        ExprKind::CXXNullPtrLiteralExpr => out.push_str("nullptr"),
        _ => match props::eval_int(ast, e) {
            Some(v) => {
                let _ = write!(out, "{v}");
            }
            None => out.push_str("<expr>"),
        },
    }
}

/// clang's `TypePrinter` under the C printing policy, for the M2 kinds.
#[derive(Debug)]
struct CPrinter<'a> {
    ast: &'a Ast,
    out: String,
    /// clang's `HasEmptyPlaceHolder`.
    empty_ph: bool,
    depth: u32,
}

impl<'a> CPrinter<'a> {
    fn new(ast: &'a Ast) -> Self {
        CPrinter { ast, out: String::new(), empty_ph: true, depth: 0 }
    }

    fn print_into(mut self, q: QualType, out: &mut String) {
        self.print(q, "");
        out.push_str(&self.out);
    }

    fn push(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn space_before_ph(&mut self) {
        if !self.empty_ph {
            self.out.push(' ');
        }
    }

    fn print(&mut self, q: QualType, placeholder: &str) {
        let saved = self.empty_ph;
        self.empty_ph = placeholder.is_empty();
        self.before(q);
        self.out.push_str(placeholder);
        self.after(q);
        self.empty_ph = saved;
    }

    /// `q` with `QualifiedArray` sugar folded into its qualifiers.
    fn split(&self, q: QualType) -> (TypeId, Quals) {
        let ctx = &self.ast.types;
        let (mut t, mut quals) = (q.ty(), q.quals());
        while let TypeView::QualifiedArray { quals: qa, base } = ctx.view(t) {
            if base.index() >= t.index() {
                break;
            }
            t = base;
            quals |= qa;
        }
        (t, quals)
    }

    fn is_array_node(&self, q: QualType) -> bool {
        let (t, _) = self.split(q);
        self.ast.types.kind(t).is_array()
    }

    fn can_prefix_quals(&self, t: TypeId) -> bool {
        let ctx = &self.ast.types;
        let mut t = t;
        for _ in 0..MAX_DEPTH {
            match ctx.view(t) {
                TypeView::QualifiedArray { base, .. } => t = base,
                TypeView::ConstantArray(a) => t = a.elem.ty(),
                TypeView::IncompleteArray(e) | TypeView::VariableArray { elem: e, .. } => {
                    t = e.ty();
                }
                v => {
                    return !matches!(
                        v,
                        TypeView::Pointer(_)
                            | TypeView::LValueRef(_)
                            | TypeView::RValueRef(_)
                            | TypeView::MemberPointer { .. }
                            | TypeView::Vector { .. }
                            | TypeView::Function(_)
                    );
                }
            }
        }
        true
    }

    fn before(&mut self, q: QualType) {
        if self.depth >= MAX_DEPTH {
            self.push("...");
            return;
        }
        self.depth += 1;
        let (t, quals) = self.split(q);
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
        self.depth -= 1;
    }

    fn after(&mut self, q: QualType) {
        if self.depth >= MAX_DEPTH {
            return;
        }
        self.depth += 1;
        let (t, _) = self.split(q);
        self.after_kind(t);
        self.depth -= 1;
    }

    #[allow(clippy::too_many_lines, reason = "one arm per type kind, as clang's TypePrinter")]
    fn before_kind(&mut self, t: TypeId) {
        let ast = self.ast;
        match ast.types.view(t) {
            TypeView::Builtin(b) => {
                self.push(match b {
                    BuiltinType::Bool => "_Bool",
                    BuiltinType::NullPtr => "nullptr_t",
                    b => b.name(),
                });
                self.space_before_ph();
            }
            TypeView::BitInt { signed, width } => {
                if !signed {
                    self.push("unsigned ");
                }
                let _ = write!(self.out, "_BitInt({width})");
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
                // A bare class type: tag keyword and scope (C policy).
                match ast.types.canonical_view(QualType::unqual(class)) {
                    TypeView::Record(d) => {
                        let mut s = String::new();
                        write_tag_type(ast, d, &mut s);
                        self.push(&s);
                    }
                    _ => self.delegate(QualType::unqual(class), true),
                }
                self.push("::*");
                self.empty_ph = saved;
            }
            TypeView::ConstantArray(a) => self.before(a.elem),
            TypeView::IncompleteArray(e) | TypeView::VariableArray { elem: e, .. } => {
                self.before(e);
            }
            TypeView::Function(proto) => {
                let saved = self.empty_ph;
                self.empty_ph = false;
                self.before(proto.ret());
                self.empty_ph = saved;
                if !saved {
                    self.push("(");
                }
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                let mut s = String::new();
                write_decl_name(ast, d, false, &mut s);
                self.push(&s);
                self.space_before_ph();
            }
            TypeView::Typedef { decl, .. } | TypeView::Using { shadow: decl, .. } => {
                let mut s = String::new();
                write_decl_name(ast, decl, false, &mut s);
                self.push(&s);
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
                        let _ =
                            write!(self.out, "__attribute__((__vector_size__({count} * sizeof(");
                        self.print(elem, "");
                        self.push(")))) ");
                    }
                    VectorKind::Neon | VectorKind::NeonPoly => {
                        self.push(if kind == VectorKind::Neon {
                            "__attribute__((neon_vector_type("
                        } else {
                            "__attribute__((neon_polyvector_type("
                        });
                        let _ = write!(self.out, "{count}))) ");
                    }
                    VectorKind::Ext => {}
                }
                self.before(elem);
            }
            TypeView::Decltype { expr, .. } => {
                self.push("decltype(");
                let mut s = String::new();
                write_expr(ast, expr, &mut s);
                self.push(&s);
                self.push(")");
                self.space_before_ph();
            }
            _ => self.delegate(QualType::unqual(t), false),
        }
    }

    fn after_kind(&mut self, t: TypeId) {
        let ast = self.ast;
        match ast.types.view(t) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                self.declarator_after(p);
            }
            TypeView::MemberPointer { pointee, .. } => self.declarator_after(pointee),
            TypeView::ConstantArray(a) => {
                let _ = write!(self.out, "[{}]", a.size);
                self.after(a.elem);
            }
            TypeView::IncompleteArray(e) => {
                self.push("[]");
                self.after(e);
            }
            TypeView::Function(_) => self.function_after(t),
            TypeView::Complex(e) => self.after(e),
            TypeView::Vector { elem, count, kind } => {
                self.after(elem);
                if kind == VectorKind::Ext {
                    let _ = write!(self.out, " __attribute__((ext_vector_type({count})))");
                }
            }
            _ => {}
        }
    }

    /// Prints a kind this printer does not port with kiln's own printer
    /// (C++ policy) as a leaf.
    fn delegate(&mut self, q: QualType, scoped: bool) {
        let names = AbiNames { ast: self.ast };
        let opts = PrintOptions { full_scope: scoped, ..PrintOptions::SUGARED };
        write_type(&self.ast.types, &names, q, opts, &mut self.out);
        self.space_before_ph();
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

    fn function_after(&mut self, t: TypeId) {
        let ctx = &self.ast.types;
        let Some(proto) = ctx.proto_of(t).copied() else { return };
        if !self.empty_ph {
            self.push(")");
        }
        let saved = self.empty_ph;
        self.empty_ph = false;
        self.push("(");
        let params = ctx.params(&proto).to_vec();
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
        } else if params.is_empty() {
            // C policy: `int (void)`.
            self.push("void");
        }
        self.push(")");
        let mq = proto.info().this_quals();
        if !mq.is_empty() {
            self.push(" ");
            mq.write(&mut self.out);
        }
        self.after(proto.ret());
        self.empty_ph = saved;
    }
}
