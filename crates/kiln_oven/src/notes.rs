//! Rendering VM faults into clang-worded notes: the primary note, the
//! "in call to 'f(args)'" backtrace (clang's `addCallStack`, limit 10)
//! and trailing notes ("declared here").

use std::fmt::Write as _;

use kiln_ast::dump::AstNames;
use kiln_ast::expr::ExprKind;
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::arith::{self, FloatErr, IntErr};
use crate::compile::{FState, Site};
use crate::failure::Note;
use crate::kbc::{Op, ak};
use crate::mem::{MemFault, Origin, Ptr, flags};
use crate::ty::{OTy, TyKind};
use crate::value::var_info;
use crate::vm::{Fault, value_len};
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

    fn tname(&self, ast: &Ast, t: OTy) -> String {
        Self::type_name(ast, self.types.info(t).q)
    }

    /// The notes of `fault` at `pc`, with the call stack.
    pub(crate) fn render(&self, b: &dyn SemaBridge, fault: Fault, pc: usize) -> Vec<Note> {
        let ast = b.ast();
        let site = self.site(pc);
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
            Origin::Temp(_) | Origin::Const(_) | Origin::None => s.push_str("{temporary}"),
        }
        let (mut t, mut rel) = (a.ty, u64::from(p.off));
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
                    s.push('.');
                    s.push_str(&name_of(ast, f.decl));
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
        let fr = match self.mem.slots.get(p.slot as usize) {
            Some(a) if !p.is_static() && a.frame != 0 => a.frame,
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
        };
        (site.loc, msg)
    }
}

/// The callee's spelling for "'f' evaluates to a null function pointer".
fn callee_name(ast: &Ast, call: ExprId) -> Option<String> {
    let mut e = call;
    if ast.exprs.get(e).kind() == ExprKind::CallExpr {
        e = ast.exprs.callee(e);
    }
    for _ in 0..16 {
        let k = ast.exprs.get(e).kind();
        match k {
            ExprKind::DeclRefExpr => return Some(name_of(ast, ast.exprs.decl(e))),
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            _ if k.is_cast() => e = ast.exprs.operand(e),
            ExprKind::UnaryOperator => e = ast.exprs.operand(e),
            _ => return None,
        }
    }
    None
}
