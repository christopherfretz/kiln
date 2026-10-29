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

use kiln_ast::decl::{DeclKind, DefinitionBits, NamespaceData};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{
    BuiltinType, PrintOptions, Quals, TypeNames, TypeView, VectorKind, write_type,
};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, TypeId};
use kiln_source::SourceManager;
use kiln_source::symbol::Symbol;

use crate::layout::{Layouter, props};
use crate::target::Target;

/// Nesting limit of the type printer (deeper types print `...`).
const MAX_DEPTH: u32 = 256;

/// How a dump names unnamed classes.
#[derive(Clone, Copy, Debug, Default)]
pub struct DumpOptions<'a> {
    /// The source manager the AST's locations refer to: unnamed classes
    /// print `(unnamed at F:L:C)` with the presumed location of their
    /// tag keyword. Without it they print `(unnamed)`.
    pub sources: Option<&'a SourceManager>,
    /// Name the dumped class itself as clang does when it lays the class
    /// out at its closing brace (`-fdump-record-layouts-complete`): an
    /// anonymous struct or union is still `(unnamed ...)` and a class
    /// later named by a typedef ([dcl.typedef]/9) is still unnamed.
    /// Otherwise it is named as at the end of the translation unit
    /// (clang's on-demand layout).
    pub at_definition: bool,
}

/// The `-fdump-record-layouts` block of class `record`, including the
/// leading `\n*** Dumping AST Record Layout\n` line clang emits before
/// each class.
pub fn dump_record_layout(ast: &Ast, record: DeclId, target: &Target) -> String {
    let mut lay = Layouter::new(ast, target);
    let mut out = String::new();
    write_record_layout(&mut lay, record, &mut out);
    out
}

/// The complete, valid, non-dependent class definitions of `ast` in the
/// order their closing braces were seen, the order clang's
/// `-fdump-record-layouts-complete` dumps them in: a class follows the
/// classes defined lexically inside it; otherwise classes are in
/// creation order. A class in a function body is ordered by itself
/// (member function bodies are parsed after the class's `}`).
pub fn record_definitions(ast: &Ast) -> Vec<DeclId> {
    use kiln_ast::decl::DeclFlags;
    let mut defs = Vec::new();
    let mut index = std::collections::HashMap::new();
    for d in ast.decls.ids() {
        let Some(r) = ast.decls.record(d) else { continue };
        let n = ast.decls.get(d);
        if !r.defn.contains(DefinitionBits::COMPLETE)
            || ast.definition(d) != Some(d)
            || n.has(DeclFlags::INVALID)
            || n.has(DeclFlags::DEPENDENT_CTX)
            || n.ty().is_none_or(|t| ast.types.is_dependent(t))
        {
            continue;
        }
        index.insert(d, defs.len());
        defs.push(d);
    }
    // The innermost enclosing class definition through lexical contexts
    // (stopping at anything but a class).
    let parent = |d: DeclId| -> Option<usize> {
        let p = ast.decls.lexical_ctx(d)?;
        ast.decls.record(p)?;
        index.get(&p).copied()
    };
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); defs.len()];
    let mut roots = Vec::new();
    for (i, &d) in defs.iter().enumerate() {
        match parent(d) {
            Some(p) => children[p].push(i),
            None => roots.push(i),
        }
    }
    let mut out = Vec::with_capacity(defs.len());
    let mut stack: Vec<(usize, bool)> = roots.iter().rev().map(|&i| (i, false)).collect();
    while let Some((i, expanded)) = stack.pop() {
        if expanded {
            out.push(defs[i]);
            continue;
        }
        stack.push((i, true));
        stack.extend(children[i].iter().rev().map(|&c| (c, false)));
    }
    out
}

/// The `-fdump-record-layouts-complete` output for `ast`: every class of
/// [`record_definitions`], each named as at its closing brace.
pub fn dump_all_record_layouts(
    ast: &Ast,
    target: &Target,
    sources: Option<&SourceManager>,
) -> String {
    let mut lay = Layouter::new(ast, target);
    let opts = DumpOptions { sources, at_definition: true };
    let mut out = String::new();
    for d in record_definitions(ast) {
        write_record_layout_with(&mut lay, d, &opts, &mut out);
    }
    out
}

/// One pending piece of a dump.
enum Item {
    /// A class at an absolute byte offset, indent level and description
    /// (field name or `(base)`); `None` for the outermost class.
    Record { d: DeclId, off: u64, indent: usize, desc: Option<String> },
    /// Finished text.
    Line(String),
}

/// Appends the dump block of `record` to `out`, reusing `lay`'s cache
/// (default [`DumpOptions`]).
pub fn write_record_layout(lay: &mut Layouter<'_>, record: DeclId, out: &mut String) {
    write_record_layout_with(lay, record, &DumpOptions::default(), out);
}

/// Appends the dump block of `record` to `out` (clang
/// `DumpRecordLayout`): the class, its non-virtual bases sorted by
/// offset (each expanded, without a footer), its fields (class-typed
/// ones expanded), and the size footer.
pub fn write_record_layout_with(
    lay: &mut Layouter<'_>,
    record: DeclId,
    opts: &DumpOptions<'_>,
    out: &mut String,
) {
    out.push_str("\n*** Dumping AST Record Layout\n");
    let ast = lay.ast();
    let first = ast.first(record);
    let names = Names {
        ast,
        sources: opts.sources,
        as_defined: if opts.at_definition { Some(first) } else { None },
    };
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
        write_tag_type(&names, ast.first(d), out);
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
        let mut frames = Vec::new();
        // Non-virtual bases, stably sorted by offset.
        let mut bases: Vec<(DeclId, u64)> = layout.bases.to_vec();
        bases.sort_by_key(|&(_, o)| o);
        for (b, boff) in bases {
            let desc =
                if layout.primary_base == Some((b, false)) { "(primary base)" } else { "(base)" };
            frames.push(Item::Record {
                d: b,
                off: off + boff,
                indent: inner,
                desc: Some(desc.to_owned()),
            });
        }
        let fields = props::fields(ast, def);
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
            CPrinter::new(names).print_into(f.ty, &mut line);
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
        // Frames are pushed in reverse so they pop in order.
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

/// How declarations are named in a dump (clang `TypePrinter::printTag`
/// with the default policy).
#[derive(Clone, Copy)]
pub(crate) struct Names<'a> {
    pub ast: &'a Ast,
    sources: Option<&'a SourceManager>,
    /// The class named as at its closing brace
    /// ([`DumpOptions::at_definition`]).
    as_defined: Option<DeclId>,
}

impl std::fmt::Debug for Names<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Names").field("as_defined", &self.as_defined).finish_non_exhaustive()
    }
}

impl Names<'_> {
    /// The typedef naming the unnamed class `d` for linkage purposes,
    /// unless `d` is named as at its definition.
    fn typedef_for_anon(&self, d: DeclId) -> Option<DeclId> {
        if self.as_defined == Some(self.ast.first(d)) {
            return None;
        }
        self.ast.decls.typedef_for_anon(d)
    }
}

/// A bare class type in the C policy: `struct ns::S` (the tag keyword is
/// omitted for an unnamed class named by a typedef).
fn write_tag_type(names: &Names<'_>, d: DeclId, out: &mut String) {
    let ast = names.ast;
    let keyword = names.typedef_for_anon(d).is_none();
    if keyword {
        let tag = ast.decls.record(d).map_or("struct", |r| r.tag.keyword());
        out.push_str(tag);
        out.push(' ');
    }
    write_scope(names, d, out);
    write_simple_name(names, d, keyword, out);
}

/// Writes the name of `d`, preceded by its enclosing namespaces and
/// classes when `scoped`.
pub(crate) fn write_decl_name(names: &Names<'_>, d: DeclId, scoped: bool, out: &mut String) {
    if scoped {
        write_scope(names, d, out);
    }
    write_simple_name(names, d, false, out);
}

/// The enclosing scopes of `d` (clang `TypePrinter::AppendScope`):
/// namespaces and named classes, outermost first, stopping at a
/// function; inline namespaces and linkage specifications are
/// transparent and unnamed classes print nothing.
fn write_scope(names: &Names<'_>, d: DeclId, out: &mut String) {
    let ast = names.ast;
    let mut chain = Vec::new();
    let mut c = ast.decls.get(d).ctx();
    let mut guard = 0;
    while let Some(s) = c {
        guard += 1;
        if guard > 4096 {
            break;
        }
        let kind = ast.decls.get(s).kind();
        if kind.is_function() {
            break;
        }
        match kind {
            DeclKind::Namespace => {
                let flags = ast.decls.namespace(s).map_or(0, |n| n.flags);
                if flags & NamespaceData::INLINE == 0 {
                    chain.push(s);
                }
            }
            DeclKind::CXXRecord | DeclKind::Enum => {
                if !ast.decls.get(s).name().is_empty() || names.typedef_for_anon(s).is_some() {
                    chain.push(s);
                }
            }
            _ => {}
        }
        c = ast.decls.get(s).ctx();
    }
    for s in chain.into_iter().rev() {
        write_simple_name(names, s, false, out);
        out.push_str("::");
    }
}

/// The simple name of `d`; for an unnamed tag clang's
/// `(anonymous at F:L:C)` / `(unnamed at F:L:C)`, with the tag kind
/// after `unnamed` unless the keyword was already printed
/// (`keyword_printed`).
fn write_simple_name(names: &Names<'_>, d: DeclId, keyword_printed: bool, out: &mut String) {
    let ast = names.ast;
    let node = ast.decls.get(d);
    let name = node.name();
    if !name.is_empty() {
        out.push_str(&ast.names.simple_spelling(name, ast.symbols()).unwrap_or_default());
        return;
    }
    match node.kind() {
        DeclKind::Namespace => out.push_str("(anonymous namespace)"),
        DeclKind::CXXRecord | DeclKind::Enum => {
            if let Some(t) = names.typedef_for_anon(d) {
                write_simple_name(names, t, false, out);
                return;
            }
            let rec = ast.decls.record(d);
            out.push('(');
            let as_defined = names.as_defined == Some(ast.first(d));
            match rec {
                Some(r) if r.defn.contains(DefinitionBits::LAMBDA) => out.push_str("lambda"),
                Some(r) if r.defn.contains(DefinitionBits::ANONYMOUS) && !as_defined => {
                    out.push_str("anonymous");
                }
                _ => out.push_str("unnamed"),
            }
            let lambda = rec.is_some_and(|r| r.defn.contains(DefinitionBits::LAMBDA));
            if !keyword_printed && !lambda {
                out.push(' ');
                out.push_str(rec.map_or("enum", |r| r.tag.keyword()));
            }
            if let Some(sm) = names.sources
                && let Some(p) = sm.presumed_line_col(node.loc())
            {
                let _ = write!(out, " at {}:{}:{}", p.file_name, p.line, p.col);
            }
            out.push(')');
        }
        _ => {}
    }
}

/// [`TypeNames`] over an [`Ast`] for the kinds [`CPrinter`] delegates.
#[derive(Clone, Copy, Debug)]
struct AbiNames<'a> {
    names: Names<'a>,
}

impl TypeNames for AbiNames<'_> {
    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        write_decl_name(&self.names, decl, scoped, out);
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        write_expr(self.names.ast, expr, out);
    }

    fn write_value(&self, _ty: QualType, _value: ConstValueId, out: &mut String) {
        out.push_str("<value>");
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.names.ast.symbols().as_str(sym));
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
    names: Names<'a>,
    out: String,
    /// clang's `HasEmptyPlaceHolder`.
    empty_ph: bool,
    depth: u32,
}

impl<'a> CPrinter<'a> {
    fn new(names: Names<'a>) -> Self {
        CPrinter { ast: names.ast, names, out: String::new(), empty_ph: true, depth: 0 }
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
                        write_tag_type(&self.names, d, &mut s);
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
                write_decl_name(&self.names, d, false, &mut s);
                self.push(&s);
                self.space_before_ph();
            }
            TypeView::Typedef { decl, .. } | TypeView::Using { shadow: decl, .. } => {
                let mut s = String::new();
                write_decl_name(&self.names, decl, false, &mut s);
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
        let names = AbiNames { names: self.names };
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
