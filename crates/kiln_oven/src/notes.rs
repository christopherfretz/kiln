//! Rendering VM faults into clang-worded notes: the primary note, the
//! "in call to 'f(args)'" backtrace (clang's `addCallStack`, limit 10)
//! and trailing notes ("declared here").

use std::fmt::Write as _;

use kiln_ast::decl::DeclKind;
use kiln_ast::decl::builtin as id;
use kiln_ast::dump::AstNames;
use kiln_ast::expr::{ExprFlags, ExprKind, UnaryOp};
use kiln_ast::lambda::ClosureThunk;
use kiln_ast::types::TypeNames as _;
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::arith::{self, FloatErr, IntErr};
use crate::compile::{FState, Site, designator_type, skip_parens};
use crate::failure::Note;
use crate::kbc::{Op, ak};
use crate::mem::{DynKind, MemFault, Origin, Ptr, Sp, flags};
use crate::ty::{FieldInfo, OTy, TyKind};
use crate::value::var_info;
use crate::vm::{BiFault, BoundErr, Fault, value_len};
use crate::{Oven, SemaBridge};

/// An evaluation paused in a bridge callback ([`Oven::frame_mark`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameMark {
    frames: u32,
    entry: u32,
    depth: u32,
    /// The evaluation is a potential-constant check's
    /// ([`Oven::evaluate_call`], [`Oven::evaluate_potential`], a frame
    /// job): its frames are the check's own, not the program's.
    potential: bool,
}

/// clang's default `-fconstexpr-backtrace-limit`.
pub(crate) const BACKTRACE_LIMIT: usize = 10;

/// Elements of an array argument printed before eliding (clang's
/// `APValue::printPretty`).
const MAX_ELEMS: u64 = 10;

/// clang 22's `note_constexpr_dereferencing_null`.
const NULL_DEREF_22: &str = "dereferencing a null pointer is not allowed in a constant expression";

/// Delimits a raw `Loc` in a note under construction
/// ([`Oven::render`] replaces it with `F:L:C`).
const LOC_MARK: char = '\u{1}';

/// `msg` with each [`LOC_MARK`]-delimited raw `Loc` written as clang
/// prints a declaration's location in a type name (`F:L:C`), or with
/// ` at <loc>` dropped without `sources` (as `AstNames::new` prints an
/// unnamed class).
fn resolve_locs(sources: Option<&kiln_source::SourceManager>, msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut parts = msg.split(LOC_MARK);
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    while let (Some(raw), Some(rest)) = (parts.next(), parts.next()) {
        let p = raw.parse::<u32>().ok().and_then(|r| sources?.presumed_line_col(Loc::from_raw(r)));
        match p {
            Some(p) => {
                let _ = write!(out, "{}:{}:{}", p.file_name, p.line, p.col);
            }
            None => {
                if out.ends_with(" at ") {
                    out.truncate(out.len() - 4);
                }
            }
        }
        out.push_str(rest);
    }
    out
}

fn name_of(ast: &Ast, d: DeclId) -> String {
    let mut s = String::new();
    write_name_for_diag(ast, d, &mut s);
    s
}

/// `d`'s name as clang's `getNameForDiagnostic` (unqualified): a function
/// template or variable template specialization with its arguments
/// (`f<int>`, `v<int>`).
fn write_name_for_diag(ast: &Ast, d: DeclId, out: &mut String) {
    let names = AstNames::new(ast);
    // A decomposition's variable prints as `[a, b]` (clang
    // `DecompositionDecl::printName`).
    if let Some(l) = ast.decls.decomp_bindings(d) {
        out.push('[');
        for (i, b) in ast.lists.iter::<DeclId>(l).enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            names.write_name(b, out);
        }
        out.push(']');
        return;
    }
    names.write_name(d, out);
    if let Some((_, info)) = ast.spec_info(d) {
        if ast.decls.get(d).kind().is_function() {
            kiln_ast::types::write_arg_list(&ast.types, &names, info.print_args, out);
        } else if ast.decls.var(d).is_some() {
            // A variable template specialization (`v<int>`): canonical,
            // without trailing defaulted arguments (clang
            // `VarTemplateSpecializationDecl::getNameForDiagnostic`).
            let args = ast.types.canonical_args(info.print_args);
            kiln_ast::types::write_arg_list(&ast.types, &names, args, out);
        }
    }
}

/// `d`'s name qualified by its enclosing classes and namespaces (clang's
/// `getQualifiedNameAsString`).
pub(crate) fn qualified_name(ast: &Ast, d: DeclId) -> String {
    let mut parts = vec![name_of(ast, d)];
    let mut c = ast.decls.get(d).ctx();
    while let Some(x) = c {
        let n = ast.decls.get(x);
        if !matches!(n.kind(), DeclKind::CXXRecord | DeclKind::Namespace) {
            break;
        }
        parts.push(name_of(ast, x));
        c = n.ctx();
    }
    parts.reverse();
    parts.join("::")
}

impl Oven {
    pub(crate) fn site(&self, pc: usize) -> Site {
        let i = self.sites.partition_point(|s| s.pc as usize <= pc);
        match i.checked_sub(1).map(|i| self.sites[i]) {
            Some(s) => s,
            None => Site { pc: pc as u32, loc: Loc::INVALID, e: None },
        }
    }

    fn qname(ast: &Ast, q: QualType) -> String {
        Self::type_name(ast, q)
    }

    /// ` (vector of N 'E' values)` after a quoted vector type `q`, as
    /// clang's `ConvertTypeToDiagnosticString` prints it (WP-173).
    fn vector_suffix(ast: &Ast, q: QualType) -> String {
        let kiln_ast::types::TypeView::Vector { elem, count, .. } = ast.types.canonical_view(q)
        else {
            return String::new();
        };
        let top = ast.types.desugar_top(q);
        let elem = match ast.types.view(top.ty()) {
            kiln_ast::types::TypeView::Vector { elem, .. } => elem,
            _ => elem,
        };
        let v = if count == 1 { "value" } else { "values" };
        format!(" (vector of {count} '{}' {v})", Self::type_name(ast, elem))
    }

    /// clang's `note_constexpr_invalid_void_star_cast` for a failed
    /// [expr.static.cast]/14 cast `e` of the `void *` `p`: the operand
    /// type as written (with its `aka`), the designated type `from` with
    /// the qualifiers of the complete object (or array element) it names
    /// (clang's designator type), and the target pointee as written.
    fn void_cast_note(&self, ast: &Ast, e: Option<ExprId>, from: OTy, to: OTy, p: u64) -> String {
        let cast = e.filter(|&e| ast.exprs.get(e).kind().is_cast());
        let src = cast.map_or_else(
            || "'void *'".to_owned(),
            |e| Self::quoted_aka(ast, ast.exprs.get(ast.exprs.operand(e)).ty()),
        );
        let target = cast
            .and_then(|e| {
                let top = ast.types.desugar_top(ast.exprs.get(e).ty());
                match ast.types.view(top.ty()) {
                    kiln_ast::types::TypeView::Pointer(q) => Some(q),
                    _ => None,
                }
            })
            .map_or_else(|| format!("'{}'", self.tname(ast, to)), |q| Self::quoted_aka(ast, q));
        let ptr = Ptr::decode(p);
        let mut desig = self.tname(ast, from);
        if let Some(a) = self.alloc_any(ptr)
            && let Some(dq) = match a.origin {
                Origin::Var(d) | Origin::Global(d) | Origin::External(d) => ast.decls.get(d).ty(),
                Origin::Temp(x) => Some(ast.exprs.get(x).ty()),
                Origin::Let(t) => crate::value::let_mte(ast, t)
                    .map(|x| ast.exprs.get(x).ty())
                    .or_else(|| ast.decls.get(t).ty()),
                _ => None,
            }
            && !matches!(self.types.kind(from), TyKind::Array { .. })
        {
            // Walk the designator: the qualifiers are those of the
            // declaration that names the subobject (the variable, or the
            // member), kept through array elements and dropped at a base.
            let (mut t, mut off) = (a.ty, u64::from(ptr.off));
            let mut cv = ast.types.effective_quals(dq);
            while t != from {
                match self.types.kind(t) {
                    TyKind::Array { elem, .. } => {
                        off %= self.types.size(elem).max(1);
                        t = elem;
                    }
                    TyKind::Record { .. } if let Some(f) = self.types.field_at(t, off) => {
                        cv = match ast.decls.get(f.decl).ty() {
                            Some(fq) if !f.base => ast.types.effective_quals(fq),
                            _ => kiln_ast::types::Quals::NONE,
                        };
                        off -= f.offset;
                        t = f.ty;
                    }
                    _ => break,
                }
            }
            if t == from && !cv.is_empty() {
                let q = self.types.info(from).q;
                desig = Self::type_name(ast, QualType::new(q.ty(), q.quals_bits() | cv.bits()));
            }
        }
        format!(
            "cast from {src} is not allowed in a constant expression because the pointed object type '{desig}' is not similar to the target type {target}"
        )
    }

    /// `'T'`, or `'T' (aka 'U')` when `q` is sugar clang desugars.
    pub(crate) fn quoted_aka(ast: &Ast, q: QualType) -> String {
        let s = Self::type_name(ast, q);
        match kiln_ast::types::aka(&ast.types, ast, q) {
            Some(a) => format!("'{s}' (aka '{a}')"),
            None => format!("'{s}'"),
        }
    }

    pub(crate) fn tname(&self, ast: &Ast, t: OTy) -> String {
        if let TyKind::Array { elem, n } = self.types.kind(t)
            && self.types.is_synthetic_array(t)
        {
            return array_name(&self.tname(ast, elem), n);
        }
        Self::type_name(ast, self.types.info(t).q)
    }

    /// The notes of `fault` at `pc`, with the call stack.
    pub(crate) fn render(&self, b: &dyn SemaBridge, fault: Fault, pc: usize) -> Vec<Note> {
        let mut notes = self.render_notes(b, fault, pc);
        for n in &mut notes {
            if n.msg.contains(LOC_MARK) {
                n.msg = resolve_locs(b.sources(), &n.msg);
            }
        }
        notes
    }

    /// A token for the evaluation that is paused in a bridge callback now
    /// (its call stack and nesting): sema compares it with
    /// [`Oven::paused_call_notes`]'s caller's to know the Oven it holds is
    /// still paused in that callback (WP-1248).
    pub fn frame_mark(&self) -> FrameMark {
        FrameMark {
            frames: u32::try_from(self.stack.len()).unwrap_or(u32::MAX),
            entry: u32::try_from(self.run.entry).unwrap_or(u32::MAX),
            depth: self.ctx_depth,
            potential: self.run.potential.is_some(),
        }
    }

    /// The call stack of the evaluation that was paused at `mark` (a
    /// reflection intrinsic's [`SemaBridge::meta`], [`Oven::frame_mark`])
    /// and still is, innermost first, as the "in call to 'f(args)'" notes
    /// of a failure's backtrace with the same elision
    /// (`-fconstexpr-backtrace-limit`): what sema appends to the
    /// instantiation context of an error in an instantiation the callback
    /// required (spec/oven.md "Interface", WP-1248). The evaluation may be
    /// paused under a nested one (an evaluation the callback's work
    /// started, at a greater depth, whose frames sit above the mark on
    /// the same stack): only the marked evaluation's frames print, so each
    /// paused evaluation's notes end the context at its own call (WP-1258).
    /// Empty when this Oven does not hold that evaluation, and for a
    /// potential-constant check's evaluation (sema's never-constant check,
    /// whose call frames are its own, not the program's: WP-1258).
    /// Rendered only when such an error prints.
    pub fn paused_call_notes(
        &self,
        mark: FrameMark,
        ast: &Ast,
        sources: Option<&kiln_source::SourceManager>,
    ) -> Vec<Note> {
        let mut notes = Vec::new();
        let cur = self.frame_mark();
        let held = cur == mark
            || (cur.depth > mark.depth && cur.frames >= mark.frames && cur.entry >= mark.frames);
        let (lo, hi) = (mark.entry as usize + 1, mark.frames as usize);
        if !held || mark.potential || hi <= lo || hi > self.stack.len() {
            return notes;
        }
        self.call_stack_of(ast, lo..hi, &mut notes);
        for n in &mut notes {
            if n.msg.contains(LOC_MARK) {
                n.msg = resolve_locs(sources, &n.msg);
            }
        }
        notes
    }

    fn render_notes(&self, b: &dyn SemaBridge, fault: Fault, pc: usize) -> Vec<Note> {
        if let Fault::Trap(i) = fault
            && self.traps[i as usize].msg.is_empty()
        {
            // A silent trap: the failure was already diagnosed (an
            // erroneous expression); no note, no call stack.
            return Vec::new();
        }
        if matches!(fault, Fault::Quiet) {
            return Vec::new();
        }
        let ast = b.ast();
        let mut site = self.site(pc);
        if let Fault::Trap(i) = fault
            && self.traps[i as usize].msg.is_empty()
        {
            // A silent failure (an expression sema already diagnosed, as
            // clang's contains-errors RecoveryExpr): no notes at all.
            return Vec::new();
        }
        if (matches!(fault, Fault::Callee(_))
            || self.code.get(pc).is_some_and(|&w| Op::from_u8(w as u8) == Op::VCall))
            && let Some(e) = site.e
            && matches!(
                ast.exprs.get(e).kind(),
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
            )
            && let m = skip_parens(ast, ast.exprs.callee(e))
            && ast.exprs.get(m).kind() == ExprKind::MemberExpr
        {
            // clang diagnoses a dispatch, and a call of an unusable member
            // function, at the member name.
            site.loc = ast.exprs.get(m).loc();
        }
        if let Fault::Uncaught { obj, ty, what } = &fault {
            // At the exception's throw, without a call stack (clang's
            // P3068 `note_constexpr_unhandled_exception*`).
            let (loc, note) = self.uncaught_note(ast, site, *obj, *ty, what.as_deref());
            return vec![Note::new(loc, note)];
        }
        let mut extra = Vec::new();
        let mut frame = None;
        let (loc, msg) = match fault {
            Fault::Uncaught { .. } => (site.loc, String::new()),
            Fault::NoexceptExit => (site.loc, "uncaught exception in noexcept function".into()),
            Fault::RethrowNone => (site.loc, "there is no active exception to rethrow".into()),
            Fault::ThrowWhileActive { prev } => {
                extra.push(Note::new(
                    self.site(prev as usize).loc,
                    "currently active exception thrown here",
                ));
                (
                    site.loc,
                    "exception thrown out of an object destructor while unrolling another \
                     exception"
                        .into(),
                )
            }
            Fault::Trap(i) => {
                let t = &self.traps[i as usize];
                extra.extend(t.extra.iter().cloned());
                (t.loc, t.msg.clone())
            }
            Fault::Steps(loc) => {
                (loc, "constexpr evaluation hit maximum step limit; possible infinite loop?".into())
            }
            Fault::Depth => (
                site.loc,
                format!(
                    "constexpr evaluation exceeded maximum depth of {} calls",
                    self.budget.depth
                ),
            ),
            Fault::Memory => (
                site.loc,
                format!(
                    "constexpr evaluation exceeded the memory limit of {} bytes",
                    self.budget.bytes
                ),
            ),
            Fault::Int { err, op, a, b: rhs, aux } => {
                (site.loc, self.int_msg(ast, site, err, op, a, rhs, aux))
            }
            Fault::Wide { err, text, width } => {
                // The operation's type as written (`W` keeps its sugar
                // through the usual arithmetic conversions).
                let ty = site.e.map_or_else(
                    || "'_BitInt'".into(),
                    |e| Self::quoted_aka(ast, ast.exprs.get(e).ty()),
                );
                let msg = match err {
                    IntErr::Overflow => {
                        format!("value {text} is outside the range of representable values of type {ty}")
                    }
                    IntErr::DivZero => "division by zero".into(),
                    IntErr::ShiftNeg => format!("negative shift count {text}"),
                    IntErr::ShiftBig => {
                        format!("shift count {text} >= width of type {ty} ({width} bits)")
                    }
                };
                (site.loc, msg)
            }
            Fault::Float(e) => (site.loc, self.float_msg(ast, site, e)),
            Fault::FloatRange { kind, bits } => {
                let t = site
                    .e
                    .map_or_else(|| "'int'".into(), |e| {
                        // A complex part or a vector lane: its element
                        // type (clang); otherwise the type as written,
                        // with its `aka`.
                        let q = ast.exprs.get(e).ty();
                        match ast.types.canonical_view(q) {
                            kiln_ast::types::TypeView::Vector { elem, .. } => {
                                format!("'{}'", Self::qname(ast, elem))
                            }
                            _ => match ast.types.as_complex(q) {
                                Some(el) => format!("'{}'", Self::qname(ast, el)),
                                None => Self::quoted_aka(ast, q),
                            },
                        }
                    });
                (
                    site.loc,
                    format!(
                        "value {} is outside the range of representable values of type {t}",
                        arith::apfloat_to_string(kind, bits)
                    ),
                )
            }
            Fault::Mem { f, ak: k, ty, p } => {
                let (loc, msg) = self.mem_msg(ast, site, f, k, ty, p, &mut extra);
                match self.trivial_copy_src(pc) {
                    Some((src, whole)) => {
                        let (at, call) = self.trivial_copy_frame(ast, site, src, whole);
                        frame = Some(call);
                        match self.unconstructed_base_of(ast, f, src, whole) {
                            Some(base) => base,
                            None => (at, msg),
                        }
                    }
                    None => (loc, msg),
                }
            }
            Fault::NullArith => {
                (site.loc, "cannot perform pointer arithmetic on null pointer".into())
            }
            // clang designates a variable length array as an array of
            // unknown bound: the arithmetic succeeds and the access fails
            // (`note_constexpr_access_unsized_array`). Kiln faults at the
            // arithmetic, and reports it as the (usual) read.
            Fault::ArrayIndex { .. } if site.e.is_some_and(|e| indexes_vla(ast, e)) => (
                site.loc,
                "read of element of array without known bound is not allowed in a constant expression"
                    .into(),
            ),
            Fault::ArrayIndex { idx, n, array } => {
                let what = if array {
                    format!("array of {n} element{}", if n == 1 { "" } else { "s" })
                } else {
                    "non-array object".into()
                };
                (
                    site.loc,
                    format!("cannot refer to element {idx} of {what} in a constant expression"),
                )
            }
            Fault::Invalid => (site.loc, "subexpression not valid in a constant expression".into()),
            Fault::EnumRange { v, min, max } => {
                let t = site.e.map_or_else(String::new, |e| Self::type_name(ast, ast.exprs.get(e).ty()));
                (
                    site.loc,
                    format!(
                        "integer value {v} is outside the valid range of values [{min}, {max}] \
                         for the enumeration type '{t}'"
                    ),
                )
            }
            Fault::Quiet => (site.loc, String::new()),
            // clang 22 note_constexpr_pointer_arith_unspecified; clang 19's
            // generic note.
            Fault::PtrArith { p, q, pointee } if self.clang_compat >= 22 => (
                site.loc,
                format!(
                    "arithmetic involving unrelated objects '{}' and '{}' has unspecified value",
                    self.ptr_string(ast, p, pointee),
                    self.ptr_string(ast, q, pointee)
                ),
            ),
            Fault::PtrArith { .. } => {
                (site.loc, "subexpression not valid in a constant expression".into())
            }
            Fault::ZeroSizeSub { elem } => (
                site.loc,
                format!("subtraction of pointers to type '{}' of zero size", self.tname(ast, elem)),
            ),
            Fault::MpVirtCmp { member } => (
                site.loc,
                format!(
                    "comparison of pointer to virtual member function '{}' has unspecified value",
                    name_of(ast, member)
                ),
            ),
            Fault::PtrSub => {
                (site.loc, "subtracted pointers are not elements of the same array".into())
            }
            Fault::PtrCmp { p, q, pointee } if self.clang_compat < 22 => (
                site.loc,
                format!(
                    "comparison between '{}' and '{}' has unspecified value",
                    self.ptr_string(ast, p, pointee),
                    self.ptr_string(ast, q, pointee)
                ),
            ),
            // clang note_constexpr_pointer_comparison_unspecified (P2280;
            // clang 22 for every comparison of unrelated objects).
            Fault::PtrCmpUnknown { p, q, pointee } | Fault::PtrCmp { p, q, pointee } => (
                site.loc,
                format!(
                    "comparison between pointers to unrelated objects '{}' and '{}' has unspecified value",
                    self.ptr_string(ast, p, pointee),
                    self.ptr_string(ast, q, pointee)
                ),
            ),
            Fault::PtrPastCmp { p, pointee } => (
                site.loc,
                format!(
                    "comparison against pointer '{}' that points past the end of a complete object has unspecified value",
                    self.ptr_string(ast, p, pointee)
                ),
            ),
            Fault::WeakCmp { p, pointee } => (
                site.loc,
                format!(
                    "comparison against address of weak declaration '{}' can only be performed at runtime",
                    self.ptr_string(ast, p, pointee)
                ),
            ),
            Fault::LiteralCmp if self.clang_compat >= 22 => (
                site.loc,
                "comparison of addresses of potentially overlapping literals has unspecified value"
                    .into(),
            ),
            Fault::LiteralCmp => {
                (site.loc, "comparison of addresses of literals has unspecified value".into())
            }
            Fault::NullDeref => (site.loc, NULL_DEREF_22.into()),
            Fault::NullField => (site.loc, "cannot access field of null pointer".into()),
            Fault::NullBase => (site.loc, "cannot access base class of null pointer".into()),
            Fault::BadDowncast { dynamic, to } => (
                site.loc,
                format!(
                    "cannot cast object of dynamic type '{}' to type '{}'",
                    self.tname(ast, dynamic),
                    self.tname(ast, to)
                ),
            ),
            Fault::PastField => {
                (site.loc, "cannot access field of pointer past the end of object".into())
            }
            Fault::Union { ak: k, field, active } => {
                let what = match active {
                    Some(a) => format!("active member '{}'", name_of(ast, a)),
                    None => "no active member".into(),
                };
                (
                    site.loc,
                    format!(
                        "{} member '{}' of union with {what} is not allowed in a constant expression",
                        ak::verb(k),
                        name_of(ast, field)
                    ),
                )
            }
            Fault::Uninit { field, ty, src, whole } => {
                let (loc, call) = self.trivial_copy_frame(ast, site, src, whole);
                frame = Some(call);
                let named = field.filter(|&f| !ast.decls.get(f).name().is_empty());
                let msg = if let Some(f) = named {
                    extra.push(Note::new(ast.decls.get(f).loc(), "subobject declared here"));
                    let mut name = String::new();
                    AstNames::new(ast).write_name(f, &mut name);
                    format!("subobject '{name}' is not initialized")
                } else {
                    let t = Self::type_name(ast, self.types.info(ty).q);
                    format!("subobject of type '{t}' is not initialized")
                };
                (loc, msg)
            }
            Fault::Mutable { ak: k, field } => {
                extra.push(Note::new(ast.decls.get(field).loc(), "declared here"));
                // A trivial copy's read carries its call frame, as for
                // `Fault::Mem` (clang `handleTrivialCopy` runs in a frame).
                let mut loc = site.loc;
                if let Some((src, whole)) = self.trivial_copy_src(pc) {
                    let (at, call) = self.trivial_copy_frame(ast, site, src, whole);
                    frame = Some(call);
                    loc = at;
                }
                (
                    loc,
                    format!(
                        "{} mutable member '{}' is not allowed in a constant expression",
                        ak::verb(k),
                        name_of(ast, field)
                    ),
                )
            }
            // clang note_constexpr_access_volatile_obj and
            // note_constexpr_volatile_here ([expr.const]/10.8, WP-1019).
            Fault::Volatile { ak: k, member, p } => {
                let mut loc = site.loc;
                if let Some((src, whole)) = self.trivial_copy_src(pc) {
                    let (at, call) = self.trivial_copy_frame(ast, site, src, whole);
                    frame = Some(call);
                    loc = at;
                }
                let origin = self.alloc_any(Ptr::decode(p)).map(|a| a.origin);
                let what = match (member, origin) {
                    (Some(f), _) => {
                        extra.push(Note::new(ast.decls.get(f).loc(), "volatile member declared here"));
                        format!("member '{}'", name_of(ast, f))
                    }
                    (None, Some(Origin::Var(d))) => {
                        extra.push(Note::new(ast.decls.get(d).loc(), "volatile object declared here"));
                        format!("object '{}'", name_of(ast, d))
                    }
                    (None, o) => {
                        // A heap object's note has no location (clang's
                        // `DynamicAllocLValue` base is no expression).
                        let at = match o {
                            Some(Origin::Temp(e)) => ast.exprs.get(e).loc(),
                            _ => Loc::INVALID,
                        };
                        extra.push(Note::new(at, "volatile temporary created here"));
                        "temporary".to_owned()
                    }
                };
                (loc, format!("{} volatile {what} is not allowed in a constant expression", ak::verb(k)))
            }
            Fault::VoidCast { from, to, p } => (site.loc, self.void_cast_note(ast, site.e, from, to, p)),
            Fault::NullCallee => {
                let callee =
                    site.e.and_then(|e| callee_name(ast, e)).unwrap_or_else(|| "callee".into());
                (site.loc, format!("'{callee}' evaluates to a null function pointer"))
            }
            Fault::NewBound { err, v, signed, k } => {
                let v = arith::int_to_string(v, signed);
                let what = match err {
                    BoundErr::Negative => "is negative".to_owned(),
                    BoundErr::TooLarge => "is too large".to_owned(),
                    BoundErr::Limit => format!(
                        "exceeds the limit ({}); use '-fconstexpr-steps' to increase this limit",
                        self.budget.steps
                    ),
                    BoundErr::TooSmall => {
                        format!("is too small to hold {k} explicitly initialized elements")
                    }
                };
                (site.loc, format!("cannot allocate array; evaluated array bound {v} {what}"))
            }
            Fault::Bi(BiFault::Lifetime { kind }) => self.lifetime_note(ast, site, kind),
            Fault::Bi(f) => (site.loc, self.bi_msg(ast, site, f)),
            Fault::PureVirtual(d) => {
                extra.push(Note::new(ast.decls.get(d).loc(), "declared here"));
                (site.loc, format!("pure virtual function '{}' called", qualified_name(ast, d)))
            }
            Fault::NewBadSize { bytes, elem } => (
                site.loc,
                format!(
                    "allocated size {bytes} is not a multiple of size {} of element type '{}'",
                    self.types.size(elem),
                    self.tname(ast, elem)
                ),
            ),
            Fault::PlacementType { from, to } => (
                site.loc,
                format!(
                    "placement new would change type of storage from '{}' to '{}'",
                    self.tname(ast, from),
                    self.tname(ast, to)
                ),
            ),
            Fault::DeleteNonVirtual { stat, dynamic } => (
                site.loc,
                format!(
                    "delete of object with dynamic type '{}' through pointer to base class type '{}' with non-virtual destructor",
                    self.tname(ast, dynamic),
                    self.tname(ast, stat)
                ),
            ),
            Fault::DynCast { kind, stat, dynamic, target } => {
                let (st, dy, ta) =
                    (self.tname(ast, stat), self.tname(ast, dynamic), self.tname(ast, target));
                let what = match kind {
                    0 => format!(
                        "static type '{st}' of operand is a non-public base class of dynamic type '{dy}'"
                    ),
                    1 => format!(
                        "dynamic type '{dy}' of operand does not have a base class of type '{ta}'"
                    ),
                    2 => format!(
                        "'{ta}' is an ambiguous base class of dynamic type '{dy}' of operand"
                    ),
                    _ => format!(
                        "'{ta}' is a non-public base class of dynamic type '{dy}' of operand"
                    ),
                };
                (site.loc, format!("reference dynamic_cast failed: {what}"))
            }
            Fault::DestroyEnded { p, ty } => {
                let d = self.ptr_string(ast, p, ty);
                // clang diagnoses a variable's destruction at its
                // declaration, other objects' at the destroying expression.
                let at = match self.mem.alloc_any(Ptr::decode(p)).map(|a| a.origin) {
                    Some(Origin::Var(v)) => ast.decls.get(v).loc(),
                    _ => site.loc,
                };
                (
                    at,
                    format!(
                        "destroying object '{}' whose lifetime has already ended",
                        d.trim_start_matches('&')
                    ),
                )
            }
            Fault::NoDynamic(k) => (
                site.loc,
                format!(
                    "{} object outside its lifetime is not allowed in a constant expression",
                    ak::verb(k)
                ),
            ),
            Fault::UnknownDynamic { ak: k, p, ty } => {
                let d = self.ptr_string(ast, p, ty);
                (
                    site.loc,
                    format!(
                        "{} object '{}' whose dynamic type is not constant",
                        ak::poly_verb(k),
                        d.trim_start_matches('&')
                    ),
                )
            }
            Fault::Callee(f) if self.funcs[f as usize].state == FState::Invalid => {
                (site.loc, "subexpression not valid in a constant expression".into())
            }
            Fault::Callee(f) => {
                let func = &self.funcs[f as usize];
                let mut d = func.decl.unwrap_or_else(|| DeclId::new(0));
                let mut what =
                    if func.state == FState::NotConstexpr { "non-constexpr" } else { "undefined" };
                // An inheriting constructor whose inherited constructor is
                // not constexpr is diagnosed as that constructor (clang
                // `CheckConstexprFunction`).
                let mut inh = ast
                    .decls
                    .inheriting_ctor_shadow(d)
                    .and_then(|s| ast.using_target(s))
                    .map(|t| ast.first(t));
                if let Some(t) = inh
                    && !ast.decls.function(t).is_some_and(|f| {
                        f.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None
                    })
                {
                    d = t;
                    what = "non-constexpr";
                    inh = None;
                }
                extra.push(Note::new(ast.decls.get(ast.latest(d)).loc(), "declared here"));
                let msg = if let Some(t) = inh {
                    let base = ast.decls.get(t).ctx().map(|c| name_of(ast, c)).unwrap_or_default();
                    format!(
                        "constructor inherited from base class '{base}' cannot be used in a \
                         constant expression; derived class cannot be implicitly initialized"
                    )
                } else {
                    let kind = if ast.decls.get(d).kind() == DeclKind::CXXConstructor {
                        "constructor"
                    } else {
                        "function"
                    };
                    format!(
                        "{what} {kind} '{}' cannot be used in a constant expression",
                        name_of(ast, d)
                    )
                };
                (site.loc, msg)
            }
        };
        let mut notes = Vec::with_capacity(2 + extra.len());
        notes.push(Note::new(loc, msg));
        notes.extend(frame);
        self.call_stack(ast, &mut notes);
        notes.extend(extra);
        notes
    }

    /// The source pointer and type of the trivial copy/move constructor
    /// or assignment call that `pc` runs inline (an `Op::Copy` with aux
    /// bit 4), if it is one.
    fn trivial_copy_src(&self, pc: usize) -> Option<(u64, OTy)> {
        let w = *self.code.get(pc)?;
        if Op::from_u8(w as u8) != Op::Copy || (w >> 8) & 4 == 0 {
            return None;
        }
        let fb = self.stack.last()?.fb as usize;
        let src = *self.regs.get(fb + *self.code.get(pc + 2)? as usize)?;
        Some((src as u64, OTy(*self.code.get(pc + 3)?)))
    }

    /// A trivial copy of the `ty` object at `src` that failed (`f`) on
    /// the bytes of a base its constructor never constructed
    /// (`Memory::set_unconstructed`): clang copies the absent base and
    /// its `CheckFullyInitialized` notes "constructor of base class 'B'
    /// is not called" at the base-specifier, as a frozen result's is
    /// (`value.rs`, WP-390): the first such base, bases before members
    /// (WP-409).
    fn unconstructed_base_of(
        &self,
        ast: &Ast,
        f: MemFault,
        src: u64,
        ty: OTy,
    ) -> Option<(Loc, String)> {
        if f != MemFault::Lifetime || self.mem.unconstructed.is_empty() {
            return None;
        }
        let p = Ptr::decode(src);
        let dead: Vec<(u64, OTy)> = self
            .mem
            .unconstructed
            .iter()
            .filter(|&&(b, _, empty)| {
                let q = Ptr::decode(b);
                !empty && q.slot == p.slot && q.generation == p.generation
            })
            .map(|&(b, bt, _)| (u64::from(Ptr::decode(b).off), bt))
            .collect();
        let (decl, bt) = self.dead_base_in(ty, u64::from(p.off), &dead, 0)?;
        let canon = ast.types.canonical(self.types.info(bt).q).ty();
        let s = ast
            .decls
            .bases(decl, &ast.lists)
            .find(|s| !s.is_virtual && ast.types.canonical(s.ty).ty() == canon)?;
        let name = Self::type_name(ast, self.types.info(bt).q);
        Some((s.loc, format!("constructor of base class '{name}' is not called")))
    }

    /// The class (and the type) of the first base among `dead` (offset,
    /// type) within the record `ty` at offset `off`: its bases, then its
    /// members, depth-first.
    fn dead_base_in(
        &self,
        ty: OTy,
        off: u64,
        dead: &[(u64, OTy)],
        depth: u32,
    ) -> Option<(DeclId, OTy)> {
        let TyKind::Record { decl, union: false, .. } = self.types.kind(ty) else { return None };
        if depth > 64 {
            return None;
        }
        let fields = self.types.fields(ty);
        for fi in fields.iter().filter(|fi| fi.base) {
            if dead.contains(&(off + fi.offset, fi.ty)) {
                return Some((decl, fi.ty));
            }
        }
        fields.iter().find_map(|fi| self.dead_base_in(fi.ty, off + fi.offset, dead, depth + 1))
    }

    /// A trivial copy constructor or assignment operator runs inline (an
    /// `Op::Copy` at its call, no frame), but clang evaluates it as a
    /// call: its failure is noted at the source argument, followed by
    /// the call's own frame note (`in call to 'S(s)'`,
    /// `in call to 't.operator=(s)'`) at the call site.
    fn trivial_copy_frame(&self, ast: &Ast, site: Site, src: u64, ty: OTy) -> (Loc, Note) {
        let arg = self.ptr_string(ast, src, ty);
        let arg = arg.strip_prefix('&').unwrap_or(&arg);
        let mut s = String::from("in call to '");
        let mut at = site.loc;
        match site.e.map(|e| (e, ast.exprs.get(e).kind())) {
            Some((e, ExprKind::CXXConstructExpr)) => {
                write_name_for_diag(ast, ast.exprs.constructor(e), &mut s);
            }
            Some((e, k @ (ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr))) => {
                let args: Vec<ExprId> = ast.lists.iter::<ExprId>(ast.exprs.call_args(e)).collect();
                let src_arg = if k == ExprKind::CXXOperatorCallExpr {
                    if let Some(&obj) = args.first() {
                        ast.write_expr(obj, &mut s);
                        s.push('.');
                    }
                    args.get(1)
                } else {
                    let callee = skip_parens(ast, ast.exprs.callee(e));
                    if ast.exprs.get(callee).kind() == ExprKind::MemberExpr {
                        ast.write_expr(ast.exprs.operand(callee), &mut s);
                        let arrow = ast.exprs.get(callee).has(ExprFlags::ARROW);
                        s.push_str(if arrow { "->" } else { "." });
                    }
                    args.first()
                };
                s.push_str("operator=");
                if let Some(&a) = src_arg {
                    at = ast.exprs.get(a).loc();
                }
            }
            _ => {}
        }
        let _ = write!(s, "({arg})'");
        (at, Note::new(site.loc, s))
    }

    /// The note of a failed `__builtin_is_within_lifetime` (clang
    /// `EvaluateBuiltinIsWithinLifetime`'s `Error`): called from the body
    /// of `std::is_within_lifetime`, it is named so and noted at that
    /// function's call.
    fn lifetime_note(&self, ast: &Ast, site: Site, kind: u8) -> (Loc, String) {
        let from_std =
            self.stack.last().filter(|_| self.stack.len() > self.run.entry + 1).filter(|fr| {
                self.funcs[fr.func as usize]
                    .decl
                    .is_some_and(|d| crate::compile::is_std_function(ast, d, "is_within_lifetime"))
            });
        let (loc, name) = match from_std {
            Some(fr) => (self.site(fr.call_pc as usize).loc, "std::is_within_lifetime"),
            None => (site.loc, "__builtin_is_within_lifetime"),
        };
        let what = match kind {
            0 => "a null pointer",
            1 => "a function pointer",
            2 => "a one-past-the-end pointer",
            _ => "a pointer to an object whose lifetime has not yet begun",
        };
        (loc, format!("'{name}' cannot be called with {what}"))
    }

    /// The note of a library builtin's failure (clang's
    /// `note_constexpr_memcpy_*`, `note_constexpr_mem{cmp,chr}_unsupported`).
    /// A designated type is printed as the argument's static pointee (with
    /// its qualifiers, as clang's designator type) when it has one.
    fn bi_msg(&self, ast: &Ast, site: Site, f: BiFault) -> String {
        match f {
            BiFault::CastBitfield => {
                return "constexpr bit_cast involving bit-field is not yet supported".into();
            }
            BiFault::CastIndet { t } => {
                let chr = if ast.types.target().char_signed { " or" } else { ", 'char', or" };
                return format!(
                    "indeterminate value can only initialize an object of type 'unsigned char'{chr} 'std::byte'; '{}' is invalid",
                    self.tname(ast, t)
                );
            }
            BiFault::CastUnrep { v } => {
                return format!("value {v} cannot be represented in type 'bool'");
            }
            BiFault::Align { kind, v, align } => {
                let bytes = |n: i64| if n == 1 { "byte" } else { "bytes" };
                let a = format!("{align} {}", bytes(i64::try_from(align).unwrap_or(0)));
                return match kind {
                    0 => format!(
                        "alignment of the base pointee object ({v} {}) is less than the asserted {a}",
                        bytes(v)
                    ),
                    1 => format!(
                        "offset of the aligned pointer from the base pointee object ({v} {}) is not a multiple of the asserted {a}",
                        bytes(v)
                    ),
                    _ => format!(
                        "value of the aligned pointer ({v}) is not a multiple of the asserted {a}"
                    ),
                };
            }
            BiFault::AlignRun { adjust: false, align } => {
                return format!(
                    "cannot constant evaluate whether run-time alignment is at least {align}"
                );
            }
            BiFault::AlignRun { adjust: true, align } => {
                return format!(
                    "cannot constant evaluate the result of adjusting alignment to {align}"
                );
            }
            BiFault::AlignArg { lo, hi, w, signed, big } => {
                let b = u128::from(hi) << 64 | u128::from(lo);
                let v = if signed { (b as i128).to_string() } else { b.to_string() };
                if !big {
                    return format!("requested alignment {v} is not a positive power of two");
                }
                let max = 1u128 << (u32::from(w).clamp(1, 128) - 1);
                let src = site
                    .e
                    .and_then(|e| ast.lists.get(ast.exprs.call_args(e)).first().copied())
                    .map_or_else(String::new, |a| {
                        Self::qname(ast, ast.exprs.get(ExprId::new(a)).ty())
                    });
                return format!(
                    "requested alignment must be {max} or less for type '{src}'; {v} is invalid"
                );
            }
            _ => {}
        }
        let arg_ty = |i: usize, t: OTy| {
            site.e
                .and_then(|e| {
                    let args = ast.lists.get(ast.exprs.call_args(e));
                    args.get(i).and_then(|&a| designator_type(ast, ExprId::new(a)))
                })
                .map_or_else(|| self.tname(ast, t), |q| Self::qname(ast, q))
        };
        // The builtin the call names: `wmemcpy`/`wmemmove` and `bcmp`
        // share the `memcpy`/`memcmp` faults (WP-150).
        let callee = site.e.and_then(|e| called_builtin(ast, e));
        let w = if matches!(callee, Some(id::WMEMCPY | id::WMEMMOVE)) { "w" } else { "" };
        let quoted = |dflt: &'static str| callee.and_then(id::info).map_or(dflt, |i| i.name);
        let op = |mv: bool| format!("{w}{}", if mv { "memmove" } else { "memcpy" });
        let end = |src: bool| if src { "source" } else { "destination" };
        match f {
            BiFault::CopyNull { mv, src } => format!("{} of '{}' is nullptr", end(src), op(mv)),
            BiFault::CopyPun { mv, src, dst } => format!(
                "cannot constant evaluate '{}' from object of type '{}' to object of type '{}'",
                op(mv),
                arg_ty(1, src),
                arg_ty(0, dst)
            ),
            BiFault::CopyNontrivial { mv, t } => format!(
                "cannot constant evaluate '{}' between objects of non-trivially-copyable type '{}'",
                op(mv),
                arg_ty(0, t)
            ),
            BiFault::CopyOverlap => format!("'{w}memcpy' between overlapping memory regions"),
            BiFault::CopySize { mv, t, n, size } => format!(
                "'{}' not supported: size to copy ({n}) is not a multiple of size of element type '{}' ({size})",
                op(mv),
                arg_ty(0, t)
            ),
            BiFault::CopyRange { mv, src, t, n } => format!(
                "'{}' not supported: {} is not a contiguous array of at least {n} elements of type '{}'",
                op(mv),
                end(src),
                arg_ty(0, t)
            ),
            BiFault::CmpType { t1, t2 } => format!(
                "constant evaluation of '{}' between arrays of types '{}' and '{}' is not supported; only arrays of narrow character types can be compared",
                quoted("__builtin_memcmp"),
                arg_ty(0, t1),
                arg_ty(1, t2)
            ),
            BiFault::ChrType { t } => format!(
                "constant evaluation of '__builtin_memchr' on array of type '{}' is not supported; only arrays of narrow character types can be searched",
                arg_ty(0, t)
            ),
            BiFault::CastBitfield
            | BiFault::CastIndet { .. }
            | BiFault::CastUnrep { .. }
            | BiFault::Align { .. }
            | BiFault::Lifetime { .. }
            | BiFault::AlignRun { .. }
            | BiFault::AlignArg { .. } => String::new(),
        }
    }

    fn call_stack(&self, ast: &Ast, notes: &mut Vec<Note>) {
        self.call_stack_of(ast, self.run.entry + 1..self.stack.len(), notes);
    }

    /// [`Self::call_stack`] of the frames `range` of the stack.
    fn call_stack_of(&self, ast: &Ast, range: std::ops::Range<usize>, notes: &mut Vec<Note>) {
        let frames = &self.stack[range];
        let active = frames.len();
        let (mut skip0, mut skip1) = (active, active);
        let limit = self.backtrace_limit;
        if limit != 0 && limit < active {
            skip0 = limit / 2 + limit % 2;
            skip1 = active - limit / 2;
        }
        for (i, fr) in frames.iter().rev().enumerate() {
            // A subobject's destructor run by its destructor's epilogue
            // is noted at that destructor's call (clang).
            let mut at = fr.call_pc as usize;
            for outer in frames.iter().rev().skip(i + 1) {
                if !self
                    .code
                    .get(at)
                    .is_some_and(|&w| Op::from_u8(w as u8) == Op::PushCleanup && (w >> 8) & 2 != 0)
                {
                    break;
                }
                at = outer.call_pc as usize;
            }
            let loc = self.site(at).loc;
            if i >= skip0 && i < skip1 {
                if i == skip0 {
                    let n = active - limit;
                    notes.push(Note::new(
                        loc,
                        format!(
                            "(skipping {n} call{} in backtrace; use -fconstexpr-backtrace-limit=0 to see all)",
                            if n == 1 { "" } else { "s" }
                        ),
                    ));
                }
                continue;
            }
            let func = &self.funcs[fr.func as usize];
            let mut s = String::from("in call to '");
            // A closure's static invoker runs as its call operator with no
            // object: clang's `describeCall` then prints the first argument
            // as the object (`(0->operator()()`, or `()` with none).
            let mut invoker = match func.decl.and_then(|d| ast.closure_thunk(d)) {
                Some(ClosureThunk::Invoker { call_op }) => Some(call_op),
                _ => None,
            };
            if invoker.is_none() {
                self.object_prefix(ast, fr.call_pc as usize, fr.fb as usize, fr.func, &mut s);
                if let Some(d) = func.decl {
                    write_name_for_diag(ast, d, &mut s);
                }
            }
            s.push('(');
            let mut sep = false;
            for p in &func.params {
                if sep {
                    s.push_str(", ");
                }
                sep = true;
                let v = self.regs[fr.fb as usize + p.reg as usize];
                match self.types.kind(p.ty) {
                    // A reference argument prints as its referent's lvalue
                    // (clang: `f(a[1])`, not `f(&a[1])`).
                    TyKind::Ptr(pt) if p.is_ref => {
                        let r = self.ptr_string(ast, v as u64, pt);
                        s.push_str(r.strip_prefix('&').unwrap_or(&r));
                    }
                    _ => self.write_value(ast, &mut s, p.ty, v, 0),
                }
                if let Some(op) = invoker.take() {
                    s.push_str("->");
                    write_name_for_diag(ast, op, &mut s);
                    s.push('(');
                    sep = false;
                }
            }
            s.push_str(")'");
            notes.push(Note::new(loc, s));
        }
    }

    /// The implicit object of a member call in a call-stack note (clang's
    /// `describeCall`): the object expression of a member call
    /// expression (`p.`, `pp->`, `P(1).`), otherwise (a destructor) the
    /// object's lvalue (`r.`). Constructors and static members have none.
    fn object_prefix(&self, ast: &Ast, call_pc: usize, fb: usize, func: u32, s: &mut String) {
        let func = &self.funcs[func as usize];
        let (Some(d), Some(tr)) = (func.decl, func.this_reg) else { return };
        if ast.decls.get(d).kind() == DeclKind::CXXConstructor {
            return;
        }
        if let Some(ce) = self.site(call_pc).e
            && ast.exprs.get(ce).kind() == ExprKind::CXXMemberCallExpr
        {
            let callee = ast.exprs.callee(ce);
            let callee = skip_parens(ast, callee);
            if ast.exprs.get(callee).kind() == ExprKind::MemberExpr {
                // An implicit `this` prints too (`this->f(0)`).
                ast.write_expr(ast.exprs.operand(callee), s);
                s.push_str(if ast.exprs.get(callee).has(ExprFlags::ARROW) { "->" } else { "." });
                return;
            }
        }
        // A member operator call prints its first operand as written
        // (`u.s.data[0].operator=(e)`, `*p.operator+=(0)`).
        if let Some(ce) = self.site(call_pc).e
            && ast.exprs.get(ce).kind() == ExprKind::CXXOperatorCallExpr
            && let Some(&obj) = ast.lists.get(ast.exprs.call_args(ce)).first()
        {
            ast.write_expr(ExprId::new(obj), s);
            s.push('.');
            return;
        }
        let v = self.regs[fb + tr as usize] as u64;
        let Some(t) = func.this_ty else { return };
        // A temporary prints as the expression that created it (`P(9).~P()`).
        let p = Ptr::decode(v);
        if v != 0
            && !p.is_fn()
            && p.off == 0
            && let Some(a) = self.alloc_any(p)
            && let Origin::Temp(x) = a.origin
        {
            ast.write_expr(x, s);
            s.push('.');
            return;
        }
        let r = self.ptr_string(ast, v, t);
        s.push_str(r.strip_prefix('&').unwrap_or(&r));
        s.push('.');
    }

    /// Appends the value of a register of type `t` (clang's
    /// `APValue::printPretty`): aggregates are read from memory.
    fn write_value(&self, ast: &Ast, s: &mut String, t: OTy, v: u128, depth: u32) {
        match self.types.kind(t) {
            TyKind::Bool => s.push_str(if v != 0 { "true" } else { "false" }),
            TyKind::Int { signed, .. } => s.push_str(&arith::int_to_string(v, signed)),
            TyKind::Float(k) => s.push_str(&arith::float_arg_string(k, v)),
            TyKind::NullPtr => s.push_str("nullptr"),
            TyKind::Reflection => match kiln_ast::ReflectionId::from_bits(v) {
                Some(r) => kiln_ast::consts::PrintEnv::reflection(ast, r, s),
                None => {
                    let _ = write!(s, "^^#{v}");
                }
            },
            // A `void *` prints the subobject its tag designates, an
            // untagged one the whole object (spec/oven.md "Construction").
            TyKind::Ptr(pt) if self.types.kind(pt) == TyKind::Void => {
                let to = (v >> 64) as u32;
                let to = to.checked_sub(1).map(OTy).or_else(|| self.void_whole(v as u64));
                let to = to.unwrap_or(pt);
                s.push_str(&self.ptr_string(ast, v as u64, to));
            }
            TyKind::Ptr(pt) => s.push_str(&self.ptr_string(ast, v as u64, pt)),
            TyKind::Array { .. } | TyKind::BoolVec { .. } | TyKind::Record { .. } => {
                self.write_object(ast, s, t, v as u64, depth);
            }
            // A wide `_BitInt` register holds its object's address.
            TyKind::WideInt { .. } => self.write_object(ast, s, t, v as u64, depth),
            // clang: `&C::m` (the member's class and the member,
            // unqualified), `0` for the null member pointer.
            TyKind::MemPtr { .. } => match self.mp.get(v) {
                Some(m) => {
                    let names = AstNames::new(ast);
                    s.push('&');
                    if let Some(c) = ast.decls.get(m.member).ctx() {
                        names.write_name(c, s);
                        s.push_str("::");
                    }
                    names.write_name(m.member, s);
                }
                None => s.push('0'),
            },
            _ => s.push_str("<unknown>"),
        }
    }

    /// The note on an exception leaving the evaluation, worded as clang's
    /// P3068 implementation: "unhandled exception: W" at the
    /// throw-expression (`W` the `what()` string of a class derived from
    /// `std::exception`), else "unhandled exception of type 'T' with
    /// content V thrown from here" at its operand (`V` the object's value
    /// as clang's `APValue` prints it), without the content when the
    /// operand's type has size 1 (an empty class, a `char`, a `bool`). The type is named as the throw-expression's
    /// operand's (as written, `site.e` the throw-expression).
    /// Why the scan of an uncaught exception's `what()` string stopped
    /// before its terminating null character (`Oven::exc_what`): the
    /// note a read of that character would carry (as the `static_assert`
    /// message path prints its failure note after the characters read),
    /// without the extra notes. A fault that is not a memory access gets
    /// no reason.
    pub(crate) fn what_stop_reason(&self, ast: &Ast, f: Fault) -> Option<String> {
        let Fault::Mem { f, ak: k, ty, p } = f else { return None };
        let site = Site { pc: 0, loc: Loc::INVALID, e: None };
        Some(self.mem_msg(ast, site, f, k, ty, p, &mut Vec::new()).1)
    }

    fn uncaught_note(
        &self,
        ast: &Ast,
        site: Site,
        obj: u64,
        ty: OTy,
        what: Option<&str>,
    ) -> (Loc, String) {
        if let Some(w) = what {
            return (site.loc, format!("unhandled exception: {w}"));
        }
        let operand = site
            .e
            .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CXXThrowExpr)
            .and_then(|e| ast.exprs.throw_operand(e));
        let loc = operand.map_or(site.loc, |x| ast.exprs.get(x).loc());
        let name = operand
            .map(|x| ast.exprs.get(x).ty())
            .filter(|&q| ast.types.canonical(q).without_quals() == self.types.info(ty).q)
            .map_or_else(|| self.tname(ast, ty), |q| Self::qname(ast, q.without_quals()));
        if self.types.size(ty) == 1 {
            return (loc, format!("unhandled exception of type '{name}' thrown from here"));
        }
        let mut v = String::new();
        if self.types.is_aggregate(ty) {
            self.write_value(ast, &mut v, ty, u128::from(obj), 0);
        } else {
            match self.load(obj, ty, ak::READ) {
                Ok(x) => self.write_value(ast, &mut v, ty, x, 0),
                Err(_) => v.push_str("<unknown>"),
            }
        }
        (loc, format!("unhandled exception of type '{name}' with content {v} thrown from here"))
    }

    fn write_object(&self, ast: &Ast, s: &mut String, t: OTy, raw: u64, depth: u32) {
        if depth > 8 {
            s.push_str("{...}");
            return;
        }
        let info = *self.types.info(t);
        match info.kind {
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                s.push('{');
                for i in 0..n.min(MAX_ELEMS) {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    let r = Ptr::with_off(raw, u64::from(Ptr::decode(raw).off) + i * es, false);
                    self.write_object(ast, s, elem, r, depth + 1);
                }
                if n > MAX_ELEMS {
                    s.push_str(", ...");
                }
                s.push('}');
            }
            // clang prints every lane of a vector (`APValue::printPretty`).
            TyKind::BoolVec { elem, n } => {
                let len = n.div_ceil(8);
                let bytes = self.mem.check(raw, len, false).ok().and_then(|(st, abs, _)| {
                    let sp = self.mem.space(st);
                    sp.is_init(abs, len)
                        .then(|| sp.bytes[abs as usize..(abs + len) as usize].to_vec())
                });
                let Some(bytes) = bytes else {
                    s.push_str("<uninitialized>");
                    return;
                };
                s.push('{');
                for i in 0..n {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    let b = bytes[(i / 8) as usize] >> (i % 8) & 1;
                    self.write_value(ast, s, elem, u128::from(b), depth + 1);
                }
                s.push('}');
            }
            TyKind::Record { union, .. } => {
                let fields = self.types.fields(t);
                let off = u64::from(Ptr::decode(raw).off);
                s.push('{');
                if union {
                    if let Some(&i) = self.mem.unions.get(&Ptr::union_key(raw, off, t)) {
                        let f = fields[i as usize];
                        let _ = write!(s, ".{} = ", name_of(ast, f.decl));
                        self.write_field(ast, s, raw, f, depth);
                    }
                } else {
                    let mut first = true;
                    for f in fields {
                        if f.width == Some(0)
                            || ast.decls.get(f.decl).name().is_empty() && f.width.is_some()
                        {
                            continue;
                        }
                        if !first {
                            s.push_str(", ");
                        }
                        first = false;
                        self.write_field(ast, s, raw, *f, depth);
                    }
                }
                s.push('}');
            }
            TyKind::WideInt { signed, .. } => match self.wide_load(raw, t, ak::READ) {
                Ok(w) => s.push_str(&crate::bigint::to_decimal(&w, signed)),
                Err(_) => s.push_str("<uninitialized>"),
            },
            _ => {
                let len = value_len(info.size, info.kind);
                let v = self.mem.check(raw, len, false).ok().and_then(|(st, abs, _)| {
                    let sp = self.mem.space(st);
                    sp.is_init(abs, len).then(|| sp.read(abs, len))
                });
                match v {
                    Some(v) => {
                        let v = match info.kind {
                            TyKind::Int { width, signed } => {
                                arith::canon(v, u32::from(width), signed)
                            }
                            _ => v,
                        };
                        self.write_value(ast, s, t, v, depth + 1);
                    }
                    None => s.push_str("<uninitialized>"),
                }
            }
        }
    }

    fn write_field(&self, ast: &Ast, s: &mut String, raw: u64, f: FieldInfo, depth: u32) {
        let off = u64::from(Ptr::decode(raw).off) + f.offset;
        let r = Ptr::with_off(raw, off, false);
        let Some(w) = f.width else {
            self.write_object(ast, s, f.ty, r, depth + 1);
            return;
        };
        let n = u64::from((f.bit + w).div_ceil(8));
        let v = self.mem.check(r, n, false).ok().and_then(|(st, abs, _)| {
            let sp = self.mem.space(st);
            sp.is_init(abs, n).then(|| sp.read(abs, n))
        });
        match v {
            Some(v) => {
                let m = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
                let v = (v >> f.bit) & m;
                let v = match self.types.kind(f.ty) {
                    TyKind::Int { signed, .. } => arith::canon(v, w, signed),
                    _ => v,
                };
                self.write_value(ast, s, f.ty, v, depth + 1);
            }
            None => s.push_str("<uninitialized>"),
        }
    }

    /// The type of the whole object an untagged `void *` points into
    /// (none for a null or function pointer).
    fn void_whole(&self, raw: u64) -> Option<OTy> {
        let p = Ptr::decode(raw);
        if raw == 0 || p.is_fn() {
            return None;
        }
        self.alloc_any(p).map(|a| a.ty)
    }

    /// clang's printing of a pointer value: `&x`, `&a[1]`, `&s.f`,
    /// `&x + 1`, `&"lit"[0]`, `nullptr`, `&f`.
    pub(crate) fn ptr_string(&self, ast: &Ast, raw: u64, pointee: OTy) -> String {
        if raw == 0 {
            return "nullptr".into();
        }
        let p = Ptr::decode(raw);
        if p.is_fn() {
            return match self.funcs.get(p.off as usize).and_then(|f| f.decl) {
                Some(d) => format!("&{}", name_of(ast, d)),
                None => "&<function>".into(),
            };
        }
        let Some(a) = self.alloc_any(p) else { return "&<dead object>".into() };
        let mut s = String::from("&");
        match a.origin {
            Origin::Var(d) | Origin::Global(d) | Origin::External(d) | Origin::Unknown(d) => {
                s.push_str(&name_of(ast, d));
            }
            Origin::Str(id, _) => ast.strings.write_literal(id, &mut s),
            // clang's `APValue::printPretty`: an expression base (a
            // materialized temporary) prints as that expression, as
            // written (`h(S{3, 'b'})`, `k(3)`).
            Origin::Temp(x) => ast.write_expr(x, &mut s),
            Origin::Let(t) if let Some(x) = crate::value::let_mte(ast, t) => {
                ast.write_expr(x, &mut s);
            }
            Origin::Let(_) | Origin::Const(_) | Origin::None => {
                s.push_str("{temporary}");
            }
            // clang: `{*new T#N}`, N its allocation number.
            Origin::Heap(_, n) => {
                let _ = write!(s, "{{*new {}#{n}}}", self.tname(ast, a.ty));
            }
            Origin::TypeInfo(q) => {
                let _ = write!(s, "typeid({})", Self::type_name(ast, q));
            }
            // clang prints the `AddrLabelExpr` base as written: `&&&l`.
            Origin::Label(e) => ast.write_expr(e, &mut s),
        }
        let (mut t, mut rel) = (a.ty, u64::from(p.off));
        // clang prints a member reached through a base as `.B::m` and
        // omits a trailing base step.
        let mut via_base = None;
        loop {
            if t == pointee && rel == 0 {
                return s;
            }
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = rel / es;
                    let _ = write!(s, "[{i}]");
                    if i >= n {
                        return s;
                    }
                    rel %= es;
                    t = elem;
                }
                TyKind::Record { union, .. } => {
                    let f = if union {
                        let at = u64::from(p.off) - rel;
                        self.union_field_at(raw, at, t, rel, pointee)
                    } else {
                        self.types.field_at(t, rel)
                    };
                    let Some(f) = f else { break };
                    if f.base {
                        via_base = Some(f.decl);
                    } else {
                        s.push('.');
                        if let Some(b) = via_base.take() {
                            s.push_str(&name_of(ast, b));
                            s.push_str("::");
                        }
                        // An anonymous struct or union member prints as its
                        // type (clang `FieldDecl::printName`: `&v.V::(anonymous
                        // union at f.cpp:1:12).data[1]`), never as an empty
                        // segment (`&v..data`, WP-773).
                        let name = name_of(ast, f.decl);
                        if name.is_empty() {
                            self.write_unnamed_member(ast, f.ty, &mut s);
                        } else {
                            s.push_str(&name);
                        }
                    }
                    rel -= f.offset;
                    t = f.ty;
                }
                _ => break,
            }
        }
        if rel != 0 || p.past {
            s.push_str(" + 1");
        }
        s
    }

    /// The name of an anonymous struct or union member of type `t`: its
    /// type, `V::(anonymous union at F:L:C)`. The Oven has no source
    /// manager, so the location is a [`LOC_MARK`]-delimited raw `Loc`
    /// that [`Oven::render`] resolves through the bridge.
    fn write_unnamed_member(&self, ast: &Ast, t: OTy, s: &mut String) {
        let name = self.tname(ast, t);
        let decl = ast.types.as_record(self.types.info(t).q);
        match (decl, name.strip_suffix(')')) {
            (Some(d), Some(head))
                if head.ends_with("(anonymous union")
                    || head.ends_with("(anonymous struct")
                    || head.ends_with("(unnamed struct")
                    || head.ends_with("(unnamed union") =>
            {
                let raw = ast.decls.get(d).loc().to_raw();
                let _ = write!(s, "{head} at {LOC_MARK}{raw}{LOC_MARK})");
            }
            _ => s.push_str(&name),
        }
    }

    /// The member of union `t`, at byte `at` of `raw`'s allocation, that
    /// a pointer `rel` bytes into it designates: one with a `pointee`
    /// subobject there, the active member first (clang's designator is
    /// the path the pointer was formed through, `&u.s.data[0]`, never a
    /// sibling at the same offset).
    fn union_field_at(
        &self,
        raw: u64,
        at: u64,
        t: OTy,
        rel: u64,
        pointee: OTy,
    ) -> Option<FieldInfo> {
        let fs = self.types.fields(t);
        let holds = |f: &&FieldInfo| {
            f.width.is_none()
                && rel >= f.offset
                && self.types.has_subobject(f.ty, rel - f.offset, pointee)
        };
        let active =
            self.mem.unions.get(&Ptr::union_key(raw, at, t)).and_then(|&i| fs.get(i as usize));
        active
            .filter(holds)
            .or_else(|| fs.iter().find(holds))
            .copied()
            .or_else(|| self.types.field_at(t, rel))
    }

    fn int_msg(
        &self,
        ast: &Ast,
        site: Site,
        err: IntErr,
        op: Op,
        a: u128,
        b: u128,
        aux: u32,
    ) -> String {
        let signed = aux >> 8 & 1 != 0;
        let ty = site.e.map_or_else(
            || "int".into(),
            |e| {
                // A complex part (`++` steps the real part): its element type.
                let q = ast.exprs.get(e).ty();
                Self::qname(ast, ast.types.as_complex(q).unwrap_or(q))
            },
        );
        let vec =
            site.e.map_or_else(String::new, |e| Self::vector_suffix(ast, ast.exprs.get(e).ty()));
        match err {
            IntErr::Overflow => format!(
                "value {} is outside the range of representable values of type '{ty}'{vec}",
                arith::exact_result(op, signed, a, b)
            ),
            IntErr::DivZero => "division by zero".into(),
            IntErr::ShiftNeg => {
                let cs = aux >> 17 & 1 != 0;
                format!("negative shift count {}", arith::int_to_string(b, cs))
            }
            IntErr::ShiftBig => {
                let cs = aux >> 17 & 1 != 0;
                format!(
                    "shift count {} >= width of type '{ty}'{vec} ({} bits)",
                    arith::int_to_string(b, cs),
                    aux & 0xFF
                )
            }
        }
    }

    fn float_msg(&self, ast: &Ast, site: Site, e: FloatErr) -> String {
        match e {
            FloatErr::NaN => "floating point arithmetic produces a NaN".into(),
            FloatErr::DivZero => "division by zero".into(),
            FloatErr::Range => "floating point value out of range".into(),
            FloatErr::Unsupported => {
                let ty = site.e.map_or_else(
                    || "long double".into(),
                    |e| Self::qname(ast, ast.exprs.get(e).ty()),
                );
                format!("kiln: floating-point arithmetic on type '{ty}' is not supported")
            }
        }
    }

    /// Whether uninitialized bytes read at `p` in allocation `a` are of a
    /// subobject whose lifetime has not begun because it is not yet
    /// initialized: `a` is the object under construction of the variable
    /// whose initializer is evaluated (its name designates it, WP-1100),
    /// and no constructor or destructor of an object in it is running (in
    /// one, a member left default-initialized is "uninitialized"). clang
    /// gives a subobject its value as it is initialized, in order, so one
    /// not reached yet is absent: "outside its lifetime" (`constexpr P
    /// p{p.y, 1};`).
    fn unbegun_init_object(&self, a: &crate::mem::Alloc, p: u64) -> bool {
        let Origin::Var(d) = a.origin else { return false };
        if self.run.object != Some(d) {
            return false;
        }
        let key = Ptr::alloc_key(p);
        !self.stack.iter().any(|fr| {
            let f = &self.funcs[fr.func as usize];
            f.this_reg.filter(|_| f.structor).is_some_and(|r| {
                let this = self.regs.get((fr.fb + r) as usize).map_or(0, |&v| v as u64);
                this != 0 && Ptr::alloc_key(this) == key
            })
        })
    }

    /// Whether the call that created dead frame object `p` is running.
    fn frame_alive(&self, p: u64) -> bool {
        let p = Ptr::decode(p);
        if p.is_fn() || p.space() != Sp::Frame {
            return false;
        }
        let fr = match self.mem.slots.get(p.slot as usize) {
            Some(a) if a.frame != 0 => a.frame,
            _ => return false,
        };
        self.stack.iter().any(|f| f.serial == fr)
    }

    fn mem_msg(
        &self,
        ast: &Ast,
        site: Site,
        f: MemFault,
        k: u32,
        ty: OTy,
        p: u64,
        extra: &mut Vec<Note>,
    ) -> (Loc, String) {
        let verb = ak::verb(k);
        let msg = match f {
            // clang 22 diagnoses `*null` at the `*`; a read of `*p`
            // carries no separate check, so its site is the `*`.
            MemFault::Null
                if self.clang_compat >= 22
                    && site.e.is_some_and(|e| {
                        ast.exprs.get(e).kind() == ExprKind::UnaryOperator
                            && ast.exprs.unary_op(e) == UnaryOp::Deref
                    }) =>
            {
                NULL_DEREF_22.to_owned()
            }
            MemFault::Null => {
                format!("{verb} dereferenced null pointer is not allowed in a constant expression")
            }
            MemFault::FnPtr => format!("{verb} a function is not allowed in a constant expression"),
            // clang: a local of a still-running call whose scope ended is
            // "outside its lifetime"; one of a returned call has "ended".
            MemFault::Dead(_) if self.frame_alive(p) => {
                format!(
                    "{verb} object outside its lifetime is not allowed in a constant expression"
                )
            }
            // clang's note_constexpr_access_deleted_object (no note on the
            // allocation: clang has forgotten it). A heap slot dies only by
            // `delete`, so a stale heap pointer is to a deleted object too.
            MemFault::Dead(Origin::Heap(..)) => {
                format!("{verb} heap allocated object that has been deleted")
            }
            MemFault::Stale if is_heap_ptr(p) => {
                format!("{verb} heap allocated object that has been deleted")
            }
            // clang makes a parameter object in the caller's frame and
            // ends it on return ([expr.call]/7): "outside its lifetime",
            // with no note on the declaration (WP-1018). Kiln makes a
            // scalar one in the callee's.
            MemFault::Dead(Origin::Var(d)) if ast.decls.get(d).kind() == DeclKind::ParmVar => {
                format!(
                    "{verb} object outside its lifetime is not allowed in a constant expression"
                )
            }
            MemFault::Dead(Origin::Var(d)) => {
                extra.push(Note::new(ast.decls.get(d).loc(), "declared here"));
                format!("{verb} variable whose lifetime has ended")
            }
            MemFault::Dead(Origin::Temp(e)) => {
                extra.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
                format!("{verb} temporary whose lifetime has ended")
            }
            MemFault::Dead(_) | MemFault::Stale => {
                format!(
                    "{verb} object outside its lifetime is not allowed in a constant expression"
                )
            }
            // clang evaluates a variable length array (GNU extension) as
            // an array of unknown bound (`note_constexpr_access_unsized_array`).
            MemFault::PastEnd | MemFault::OutOfBounds
                if self.alloc_any(Ptr::decode(p)).is_some_and(|a| {
                    matches!(a.origin, Origin::Var(d)
                        if ast.decls.get(d).ty().is_some_and(|q| crate::compile::is_vla(ast, q)))
                }) =>
            {
                format!(
                    "{verb} element of array without known bound is not allowed in a constant expression"
                )
            }
            MemFault::PastEnd | MemFault::OutOfBounds => {
                format!(
                    "{verb} dereferenced one-past-the-end pointer is not allowed in a constant expression"
                )
            }
            MemFault::Uninit => {
                let resolving = self.alloc_any(Ptr::decode(p)).is_some_and(|a| {
                    a.flags & flags::RESOLVING != 0 || self.unbegun_init_object(a, p)
                });
                if resolving {
                    format!(
                        "{verb} object outside its lifetime is not allowed in a constant expression"
                    )
                } else {
                    let verb = if k == ak::COMPOUND { "read of" } else { verb };
                    format!("{verb} uninitialized object is not allowed in a constant expression")
                }
            }
            // clang 19 treats a literal as an object of unknown value.
            MemFault::Literal(e) => {
                if let Some(e) = e {
                    extra.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
                }
                let d = self.ptr_string(ast, p, ty);
                // The object's name: a literal's designator less its `&`
                // (`"a"`, `&&l`).
                let d = d.strip_prefix('&').unwrap_or(&d);
                format!("{verb} object '{d}' whose value is not known")
            }
            MemFault::Const(t) => {
                let q = self.types.info(t).q;
                let cq = QualType::new(q.ty(), q.quals_bits() | 1);
                format!(
                    "modification of object of const-qualified type '{}' is not allowed in a constant expression",
                    Self::qname(ast, cq)
                )
            }
            MemFault::ModifyGlobal | MemFault::External(_, true) => {
                "a constant expression cannot modify an object that is visible outside that expression".into()
            }
            // clang's note_constexpr_access_static_temporary.
            MemFault::Temporary(t) => {
                if let Some(e) = crate::value::let_mte(ast, t) {
                    extra.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
                }
                format!(
                    "{verb} temporary is not allowed in a constant expression outside the expression that created the temporary"
                )
            }
            // clang note_constexpr_access_unknown_variable (P2280).
            MemFault::Unknown(d) => {
                extra.push(Note::new(ast.decls.get(d).loc(), "declared here"));
                format!("{verb} variable '{}' whose value is not known", name_of(ast, d))
            }
            MemFault::External(d, false) => {
                let name = name_of(ast, d);
                {
                    let vi = var_info(ast, d);
                    // clang notes the first declaration of a parameter or
                    // a usable variable, and the definition (else the
                    // redeclaration read) of a variable that is not.
                    let parm = ast.decls.get(d).kind() == DeclKind::ParmVar;
                    let at = if parm || vi.usable { d } else { used_redecl(ast, site.e, d) };
                    let volatile = !parm && ast.types.effective_quals(vi.ty).is_volatile();
                    let here = if volatile { "volatile object declared here" } else { "declared here" };
                    extra.push(Note::new(ast.decls.get(at).loc(), here));
                    if volatile {
                        // clang note_constexpr_access_volatile_obj ([expr.const]/10.9).
                        format!("read of volatile object '{name}' is not allowed in a constant expression")
                    } else if ast.decls.compound_literal_of(d).is_some() {
                        // clang `handleLValueToRValueConversion`: a read
                        // of a non-`const` namespace-scope array literal
                        // (WP-466).
                        "subexpression not valid in a constant expression".to_owned()
                    } else if parm {
                        // clang note_constexpr_function_param_value_unknown.
                        format!(
                            "function parameter '{name}' with unknown value cannot be used in a constant expression"
                        )
                    } else if vi.usable && vi.init.is_none() {
                        format!("initializer of '{name}' is unknown")
                    } else if vi.usable {
                        format!("initializer of '{name}' is not a constant expression")
                    } else if vi.is_const || {
                        // clang `findCompleteObject`: only a non-const
                        // integral or enumeration variable is "non-const".
                        let c = ast.types.canonical(vi.ty);
                        !(ast.types.is_integral(c) || ast.types.as_enum(c).is_some())
                    } {
                        format!(
                            "read of non-constexpr variable '{name}' is not allowed in a constant expression"
                        )
                    } else {
                        format!(
                            "read of non-const variable '{name}' is not allowed in a constant expression"
                        )
                    }
                }
            }
            MemFault::Incompatible => format!(
                "kiln: {verb} object of type '{}' through a value of a different representation is not allowed in a constant expression",
                self.tname(ast, ty)
            ),
            MemFault::Memory(n) => {
                format!("constexpr evaluation exceeded the memory limit of {n} bytes")
            }
            // clang's CheckDeleteKind notes (note_constexpr_delete_not_heap_alloc,
            // _double_delete, _new_delete_mismatch, _delete_subobject).
            MemFault::NotHeap => {
                if let Some(a) = self.alloc_any(Ptr::decode(p)) {
                    match a.origin {
                        Origin::Var(d)
                        | Origin::Global(d)
                        | Origin::External(d)
                        | Origin::Unknown(d) => {
                            extra.push(Note::new(ast.decls.get(d).loc(), "declared here"));
                        }
                        Origin::Temp(e) | Origin::Str(_, Some(e)) => {
                            extra.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
                        }
                        _ => {}
                    }
                }
                format!(
                    "delete of pointer '{}' that does not point to a heap-allocated object",
                    self.ptr_string(ast, p, ty)
                )
            }
            MemFault::DoubleDelete => "delete of pointer that has already been deleted".into(),
            // Trials are silent; rendered only if one ever is not.
            MemFault::Speculative => {
                format!("kiln: {verb} an object created outside a speculative evaluation is not allowed")
            }
            MemFault::Subobject if Ptr::decode(p).past => format!(
                "delete of pointer '{}' that does not point to complete object",
                self.ptr_string(ast, p, ty)
            ),
            // clang's format leaves a trailing space here.
            MemFault::Subobject => {
                format!("delete of pointer to subobject '{}' ", self.ptr_string(ast, p, ty))
            }
            MemFault::DeleteMismatch { alloc, dealloc } => {
                let a = self.alloc_any(Ptr::decode(p)).copied();
                let t = a.map_or_else(|| self.tname(ast, ty), |a| self.tname(ast, a.ty));
                if let Some(Origin::Heap(e, _)) = a.map(|a| a.origin) {
                    extra.push(Note::new(ast.exprs.get(e).loc(), "heap allocation performed here"));
                }
                match (alloc, dealloc) {
                    (DynKind::Std, _) => "'delete' used to delete pointer to object allocated with 'std::allocator<...>::allocate'".into(),
                    (_, DynKind::Std) => "'std::allocator<...>::deallocate' used to delete pointer to object allocated with 'new'".into(),
                    (DynKind::ArrayNew, _) => format!(
                        "non-array delete used to delete pointer to array object of type '{t}'"
                    ),
                    _ => format!(
                        "array delete used to delete pointer to non-array object of type '{t}'"
                    ),
                }
            }
            MemFault::DeallocNull => {
                "'std::allocator<...>::deallocate' used to delete a null pointer".into()
            }
            MemFault::Lifetime => {
                format!("{verb} object outside its lifetime is not allowed in a constant expression")
            }
        };
        (site.loc, msg)
    }
}

/// Whether `raw` is a (possibly dangling) heap pointer.
fn is_heap_ptr(raw: u64) -> bool {
    let p = Ptr::decode(raw);
    raw != 0 && !p.is_fn() && p.is_heap()
}

/// The name of an array of `n` elements of type `elem` (named `elem`), as
/// clang prints it: `int[3]`, `int[3][2]`, `int *[3]`, `int (*[3])(int)`.
pub(crate) fn array_name(elem: &str, n: u64) -> String {
    let at = match elem.find("(*") {
        Some(i) => i + 2 + elem[i + 2..].bytes().take_while(|&c| c == b'*').count(),
        None => elem.find('[').unwrap_or(elem.len()),
    };
    format!("{}[{n}]{}", &elem[..at], &elem[at..])
}

/// Whether `e` (a subscript or pointer arithmetic) indexes a variable
/// length array variable.
fn indexes_vla(ast: &Ast, e: ExprId) -> bool {
    let e = skip_parens(ast, e);
    if !matches!(ast.exprs.get(e).kind(), ExprKind::ArraySubscriptExpr | ExprKind::BinaryOperator) {
        return false;
    }
    ast.exprs.operands(e).into_iter().any(|mut o| {
        for _ in 0..16 {
            let k = ast.exprs.get(o).kind();
            match k {
                ExprKind::DeclRefExpr => {
                    return ast
                        .decls
                        .get(ast.exprs.decl(o))
                        .ty()
                        .is_some_and(|q| crate::compile::is_vla(ast, q));
                }
                ExprKind::ParenExpr => o = ast.exprs.operand(o),
                _ if k.is_cast() => o = ast.exprs.operand(o),
                _ => return false,
            }
        }
        false
    })
}

/// The declaration of variable `d` a note on the read `e` names: its
/// definition once seen, else the redeclaration `e` names (clang
/// `findCompleteObject`: the `DeclRefExpr`'s declaration, replaced by
/// `getDefinition()`); `d` itself when `e` is not such a name.
fn used_redecl(ast: &Ast, e: Option<ExprId>, d: DeclId) -> DeclId {
    if let Some(def) = ast.redecls_of(d).find(|&v| is_var_definition(ast, v)) {
        return def;
    }
    let Some(mut e) = e else { return d };
    for _ in 0..16 {
        let k = ast.exprs.get(e).kind();
        match k {
            ExprKind::DeclRefExpr => {
                let r = ast.exprs.decl(e);
                return if ast.first(r) == ast.first(d) { r } else { d };
            }
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            _ if k.is_cast() => e = ast.exprs.operand(e),
            _ => return d,
        }
    }
    d
}

/// Whether variable declaration `v` is a definition ([basic.def]/2): one
/// with an initializer, or neither `extern` nor a static data member
/// declared in its class without `inline` nor an explicit specialization
/// of a static data member ([temp.expl.spec]/14).
fn is_var_definition(ast: &Ast, v: DeclId) -> bool {
    use kiln_ast::decl::VarStorage;
    let Some(x) = ast.decls.var(v) else { return false };
    if x.init.is_some() {
        return true;
    }
    let ctx = ast.decls.get(v).ctx();
    let in_class = ctx.is_some_and(|c| {
        ast.decls.get(c).kind() == DeclKind::CXXRecord
            && ast.decls.lexical_ctx(v).is_some_and(|l| ast.first(l) == ast.first(c))
    });
    if in_class {
        return x.spec.is_inline();
    }
    // [temp.expl.spec]/14: `template<> int C<char>::n;` declares.
    if ast.is_member_spec_declaration(v) {
        return false;
    }
    x.spec.storage() != VarStorage::Extern && !ast.decls.is_single_line_linkage(v)
}

/// The callee's spelling for "'f' evaluates to a null function pointer".
fn callee_name(ast: &Ast, call: ExprId) -> Option<String> {
    let mut e = call;
    if matches!(ast.exprs.get(e).kind(), ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr) {
        e = ast.exprs.callee(e);
    }
    for _ in 0..16 {
        let k = ast.exprs.get(e).kind();
        match k {
            ExprKind::DeclRefExpr => return Some(name_of(ast, ast.exprs.decl(e))),
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            ExprKind::SubstNonTypeTemplateParmExpr => e = ast.exprs.subst_replacement(e),
            _ if k.is_cast() => e = ast.exprs.operand(e),
            ExprKind::UnaryOperator => e = ast.exprs.operand(e),
            _ => return None,
        }
    }
    None
}

/// The builtin call `e` names directly, if any.
fn called_builtin(ast: &Ast, e: ExprId) -> Option<u32> {
    if ast.exprs.get(e).kind() != ExprKind::CallExpr {
        return None;
    }
    let mut c = skip_parens(ast, ast.exprs.callee(e));
    while ast.exprs.get(c).kind().is_cast() {
        c = skip_parens(ast, ast.exprs.operand(c));
    }
    (ast.exprs.get(c).kind() == ExprKind::DeclRefExpr)
        .then(|| ast.decls.builtin_id(ast.exprs.decl(c)))
        .flatten()
}
