//! Rendering VM faults into clang-worded notes: the primary note, the
//! "in call to 'f(args)'" backtrace (clang's `addCallStack`, limit 10)
//! and trailing notes ("declared here").

use std::fmt::Write as _;

use kiln_ast::decl::DeclKind;
use kiln_ast::dump::AstNames;
use kiln_ast::expr::{ExprFlags, ExprKind};
use kiln_ast::types::TypeNames as _;
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::arith::{self, FloatErr, IntErr};
use crate::compile::{FState, Site, designator_type, skip_parens};
use crate::failure::Note;
use crate::kbc::{Op, ak};
use crate::mem::{MemFault, Origin, Ptr, Sp, flags};
use crate::ty::{OTy, TyKind};
use crate::value::var_info;
use crate::vm::{BiFault, BoundErr, Fault, value_len};
use crate::{Oven, SemaBridge};

/// clang's default `-fconstexpr-backtrace-limit`.
pub(crate) const BACKTRACE_LIMIT: usize = 10;

/// Elements of an aggregate argument printed before eliding.
const MAX_ELEMS: u64 = 16;

fn name_of(ast: &Ast, d: DeclId) -> String {
    let mut s = String::new();
    AstNames::new(ast).write_name(d, &mut s);
    s
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
        let ast = b.ast();
        let mut site = self.site(pc);
        if self.code.get(pc).is_some_and(|&w| Op::from_u8(w as u8) == Op::VCall)
            && let Some(m) = site.e.map(|e| skip_parens(ast, ast.exprs.callee(e)))
            && ast.exprs.get(m).kind() == ExprKind::MemberExpr
        {
            // clang diagnoses a dispatch at the member name.
            site.loc = ast.exprs.get(m).loc();
        }
        let mut extra = Vec::new();
        let (loc, msg) = match fault {
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
            Fault::Float(e) => (site.loc, self.float_msg(ast, site, e)),
            Fault::FloatRange { kind, bits } => {
                let t = site
                    .e
                    .map_or_else(|| "int".into(), |e| Self::qname(ast, ast.exprs.get(e).ty()));
                (
                    site.loc,
                    format!(
                        "value {} is outside the range of representable values of type '{t}'",
                        arith::apfloat_to_string(kind, bits)
                    ),
                )
            }
            Fault::Mem { f, ak: k, ty, p } => self.mem_msg(ast, site, f, k, ty, p, &mut extra),
            Fault::NullArith => {
                (site.loc, "cannot perform pointer arithmetic on null pointer".into())
            }
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
            Fault::PtrSub => {
                (site.loc, "subtracted pointers are not elements of the same array".into())
            }
            Fault::PtrCmp { p, q, pointee } => (
                site.loc,
                format!(
                    "comparison between '{}' and '{}' has unspecified value",
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
            Fault::LiteralCmp => {
                (site.loc, "comparison of addresses of literals has unspecified value".into())
            }
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
            Fault::VoidCast { from, to } => (
                site.loc,
                format!(
                    "cast from 'void *' is not allowed in a constant expression because the pointed object type '{}' is not similar to the target type '{}'",
                    self.tname(ast, from),
                    self.tname(ast, to)
                ),
            ),
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
            Fault::Bi(f) => (site.loc, self.bi_msg(ast, site, f)),
            Fault::PureVirtual(d) => {
                extra.push(Note::new(ast.decls.get(d).loc(), "declared here"));
                (site.loc, format!("pure virtual function '{}' called", qualified_name(ast, d)))
            }
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
            Fault::Callee(f) => {
                let func = &self.funcs[f as usize];
                let d = func.decl.unwrap_or_else(|| DeclId::new(0));
                let what =
                    if func.state == FState::NotConstexpr { "non-constexpr" } else { "undefined" };
                extra.push(Note::new(ast.decls.get(ast.latest(d)).loc(), "declared here"));
                (
                    site.loc,
                    format!(
                        "{what} function '{}' cannot be used in a constant expression",
                        name_of(ast, d)
                    ),
                )
            }
        };
        let mut notes = Vec::with_capacity(2 + extra.len());
        notes.push(Note::new(loc, msg));
        self.call_stack(ast, &mut notes);
        notes.extend(extra);
        notes
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
        let op = |mv: bool| if mv { "memmove" } else { "memcpy" };
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
            BiFault::CopyOverlap => "'memcpy' between overlapping memory regions".into(),
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
                "constant evaluation of '__builtin_memcmp' between arrays of types '{}' and '{}' is not supported; only arrays of narrow character types can be compared",
                arg_ty(0, t1),
                arg_ty(1, t2)
            ),
            BiFault::ChrType { t } => format!(
                "constant evaluation of '__builtin_memchr' on array of type '{}' is not supported; only arrays of narrow character types can be searched",
                arg_ty(0, t)
            ),
            BiFault::CastBitfield | BiFault::CastIndet { .. } | BiFault::CastUnrep { .. } => {
                String::new()
            }
        }
    }

    fn call_stack(&self, ast: &Ast, notes: &mut Vec<Note>) {
        let frames = &self.stack[self.run.entry + 1..];
        let active = frames.len();
        let (mut skip0, mut skip1) = (active, active);
        let limit = self.backtrace_limit;
        if limit != 0 && limit < active {
            skip0 = limit / 2 + limit % 2;
            skip1 = active - limit / 2;
        }
        for (i, fr) in frames.iter().rev().enumerate() {
            let loc = self.site(fr.call_pc as usize).loc;
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
            self.object_prefix(ast, fr.call_pc as usize, fr.fb as usize, fr.func, &mut s);
            if let Some(d) = func.decl {
                AstNames::new(ast).write_name(d, &mut s);
            }
            s.push('(');
            for (k, p) in func.params.iter().enumerate() {
                if k > 0 {
                    s.push_str(", ");
                }
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
            TyKind::Ptr(pt) => s.push_str(&self.ptr_string(ast, v as u64, pt)),
            TyKind::Array { .. } | TyKind::Record { .. } => {
                self.write_object(ast, s, t, v as u64, depth);
            }
            _ => s.push_str("<unknown>"),
        }
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
            TyKind::Record { union, .. } => {
                let fields = self.types.fields(t);
                let off = u64::from(Ptr::decode(raw).off);
                s.push('{');
                if union {
                    if let Some(&i) = self.mem.unions.get(&Ptr::union_key(raw, off)) {
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

    fn write_field(
        &self,
        ast: &Ast,
        s: &mut String,
        raw: u64,
        f: crate::ty::FieldInfo,
        depth: u32,
    ) {
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
            Origin::Var(d) | Origin::Global(d) | Origin::External(d) => {
                s.push_str(&name_of(ast, d));
            }
            Origin::Str(id, _) => ast.strings.write_literal(id, &mut s),
            Origin::Temp(_) | Origin::Let(_) | Origin::Const(_) | Origin::None => {
                s.push_str("{temporary}");
            }
            // clang: `{*new T#N}`, N its allocation number.
            Origin::Heap(_, n) => {
                let _ = write!(s, "{{*new {}#{n}}}", self.tname(ast, a.ty));
            }
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
                TyKind::Record { .. } => {
                    let Some(f) = self.types.fields(t).iter().find(|f| {
                        f.width.is_none()
                            && rel >= f.offset
                            && rel < f.offset + self.types.size(f.ty).max(1)
                    }) else {
                        break;
                    };
                    if f.base {
                        via_base = Some(f.decl);
                    } else {
                        s.push('.');
                        if let Some(b) = via_base.take() {
                            s.push_str(&name_of(ast, b));
                            s.push_str("::");
                        }
                        s.push_str(&name_of(ast, f.decl));
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
        let ty = site.e.map_or_else(|| "int".into(), |e| Self::qname(ast, ast.exprs.get(e).ty()));
        match err {
            IntErr::Overflow => format!(
                "value {} is outside the range of representable values of type '{ty}'",
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
                    "shift count {} >= width of type '{ty}' ({} bits)",
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
            MemFault::PastEnd | MemFault::OutOfBounds => {
                format!(
                    "{verb} dereferenced one-past-the-end pointer is not allowed in a constant expression"
                )
            }
            MemFault::Uninit => {
                let resolving =
                    self.alloc_any(Ptr::decode(p)).is_some_and(|a| a.flags & flags::RESOLVING != 0);
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
                format!("{verb} object '{}' whose value is not known", d.trim_start_matches('&'))
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
            MemFault::External(d, false) => {
                extra.push(Note::new(ast.decls.get(d).loc(), "declared here"));
                let name = name_of(ast, d);
                {
                    let vi = var_info(ast, d);
                    if vi.usable && vi.init.is_none() {
                        format!("initializer of '{name}' is unknown")
                    } else if vi.usable {
                        format!("initializer of '{name}' is not a constant expression")
                    } else if vi.is_const {
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
                        Origin::Var(d) | Origin::Global(d) | Origin::External(d) => {
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
            MemFault::Subobject if Ptr::decode(p).past => format!(
                "delete of pointer '{}' that does not point to complete object",
                self.ptr_string(ast, p, ty)
            ),
            // clang's format leaves a trailing space here.
            MemFault::Subobject => {
                format!("delete of pointer to subobject '{}' ", self.ptr_string(ast, p, ty))
            }
            MemFault::DeleteMismatch { array_new } => {
                let a = self.alloc_any(Ptr::decode(p)).copied();
                let t = a.map_or_else(|| self.tname(ast, ty), |a| self.tname(ast, a.ty));
                if let Some(Origin::Heap(e, _)) = a.map(|a| a.origin) {
                    extra.push(Note::new(ast.exprs.get(e).loc(), "heap allocation performed here"));
                }
                if array_new {
                    format!("non-array delete used to delete pointer to array object of type '{t}'")
                } else {
                    format!("array delete used to delete pointer to non-array object of type '{t}'")
                }
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
