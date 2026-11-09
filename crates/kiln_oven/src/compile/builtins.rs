//! Lowering of calls of library builtins (spec/oven.md "Builtins"):
//! constants (`huge_val`, `nan`, `nans`) are folded here, the functions
//! clang 19 does not evaluate become a trap at the call, and the rest
//! become one `Op::Builtin` evaluated natively by the VM.

use kiln_ast::consts::FloatKind;
use kiln_ast::decl::builtin as id;
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::types::TypeView;
use kiln_ast::{Ast, ExprId, QualType};
use kiln_source::Loc;

use super::{Compiler, float_code, int_of, is_vla, skip_parens};
use crate::kbc::{Op, ak, bi, cmp};
use crate::softfloat;
use crate::ty::TyKind;

/// clang's `note_invalid_subexpr_in_const_expr`.
pub(super) const INVALID: &str = "subexpression not valid in a constant expression";

/// clang's `note_constexpr_new_untyped`.
pub(super) const UNTYPED_NEW: &str = "cannot allocate untyped memory in a constant expression; use 'std::allocator<T>::allocate' to allocate memory of type 'T'";

/// The type of the object the pointer argument `e` designates, as far as
/// it is known statically: the pointee of `e` with conversions to `void`
/// pointers looked through; `None` for a `void` pointer (the designated
/// object's type is then read at run time).
pub(crate) fn designator_type(ast: &Ast, mut e: ExprId) -> Option<QualType> {
    loop {
        e = skip_parens(ast, e);
        let n = ast.exprs.get(e);
        let pointee = ast.types.pointee(n.ty())?;
        if !ast.types.is_void(pointee) {
            return Some(pointee);
        }
        if !n.kind().is_cast() {
            return None;
        }
        e = ast.exprs.operand(e);
    }
}

/// clang `ignorePointerCastsAndParens` (`__builtin_object_size`): through
/// parentheses and no-op / bit casts of pointer prvalues.
fn strip_pointer_casts(ast: &Ast, e: ExprId) -> ExprId {
    use kiln_ast::expr::{CastKind, ValueCategory};
    let e = skip_parens(ast, e);
    let n = ast.exprs.get(e);
    if !n.kind().is_cast() || !matches!(ast.exprs.cast_kind(e), CastKind::NoOp | CastKind::BitCast)
    {
        return e;
    }
    let sub = ast.exprs.operand(e);
    let s = ast.exprs.get(sub);
    if ast.types.pointee(s.ty()).is_none() || s.value_category() != ValueCategory::PRValue {
        return e;
    }
    strip_pointer_casts(ast, sub)
}

/// The left operands of the commas in an object-size operand that its
/// trial skips (clang `tryEvaluateBuiltinObjectSize`, WP-302). clang
/// evaluates the operand ignoring side effects: a comma's left operand is
/// an ignored value whose failure is noted as a side effect, after which
/// any lvalue-to-rvalue read fails, and so does a call with arguments
/// (its callee reads them). A left operand with no read after it
/// in evaluation order (outside the skipped ones) is therefore skipped;
/// the others are evaluated with the operand. Commas at any depth count:
/// `(x, (y, arr))`, `&a[(f(), 1)]`, `(char *)(f(), p) + 1`.
fn skipped_comma_lhs(ast: &Ast, e: ExprId) -> Vec<ExprId> {
    use kiln_ast::expr::CastKind;
    enum Item {
        Node(ExprId, Option<usize>),
        /// The end of a comma's left operand.
        Mark(usize),
        /// A read, once its operand is evaluated.
        Read(Option<usize>),
    }
    let exprs = &ast.exprs;
    // (left operand, innermost enclosing left operand, sequence point)
    let mut commas: Vec<(ExprId, Option<usize>, u32)> = Vec::new();
    // (sequence point, innermost enclosing left operand)
    let mut reads: Vec<(u32, Option<usize>)> = Vec::new();
    let mut seq = 0u32;
    let mut stack = vec![Item::Node(e, None)];
    while let Some(it) = stack.pop() {
        let (x, par) = match it {
            Item::Mark(i) => {
                commas[i].2 = seq;
                seq += 1;
                continue;
            }
            Item::Read(par) => {
                reads.push((seq, par));
                seq += 1;
                continue;
            }
            Item::Node(x, par) => (x, par),
        };
        let n = exprs.get(x);
        match n.kind() {
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::LambdaExpr
            | ExprKind::RequiresExpr => continue,
            ExprKind::BinaryOperator if exprs.binary_op(x) == BinaryOp::Comma => {
                let [a, b] = exprs.operands(x);
                commas.push((a, par, 0));
                let i = commas.len() - 1;
                stack.extend([Item::Node(b, par), Item::Mark(i), Item::Node(a, Some(i))]);
                continue;
            }
            k if k.is_cast() && exprs.cast_kind(x) == CastKind::LValueToRValue => {
                stack.push(Item::Read(par));
            }
            // A called function reads its parameters (clang fails those
            // reads too); a call without arguments reads nothing here.
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
                if exprs.children(x, &ast.lists).len() > 1 =>
            {
                stack.push(Item::Read(par));
            }
            ExprKind::CXXConstructExpr if !exprs.children(x, &ast.lists).is_empty() => {
                stack.push(Item::Read(par));
            }
            _ => {}
        }
        stack.extend(exprs.children(x, &ast.lists).into_iter().rev().map(|c| Item::Node(c, par)));
    }
    // Later commas first: whether a read counts depends on the left
    // operands enclosing it, all of which end after it.
    let mut order: Vec<usize> = (0..commas.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(commas[i].2));
    let mut skipped = vec![false; commas.len()];
    for i in order {
        let inside_skipped = |mut p: Option<usize>| {
            while let Some(j) = p {
                if skipped[j] {
                    return true;
                }
                p = commas[j].1;
            }
            false
        };
        let read_after = reads.iter().any(|&(s, p)| s > commas[i].2 && !inside_skipped(p));
        skipped[i] = !read_after;
    }
    commas.iter().zip(skipped).filter(|&(_, s)| s).map(|(c, _)| c.0).collect()
}

/// Whether `e` calls a function not declared `const` or `pure` (the part
/// of clang `Expr::HasSideEffects` that [`has_side_effects`] leaves out).
fn has_calls(ast: &Ast, e: ExprId) -> bool {
    let exprs = &ast.exprs;
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        match exprs.get(x).kind() {
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
                if !is_pure_call(ast, x) =>
            {
                return true;
            }
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::LambdaExpr
            | ExprKind::RequiresExpr => {}
            _ => exprs.children_into(x, &ast.lists, &mut stack),
        }
    }
    false
}

/// Whether the callee of the call `e` is a function declared `const` or
/// `pure` (clang `getCalleeDecl` with a `ConstAttr` / `PureAttr`).
fn is_pure_call(ast: &Ast, e: ExprId) -> bool {
    use kiln_ast::attr::AttrKind;
    let exprs = &ast.exprs;
    let mut c = exprs.callee(e);
    for _ in 0..64 {
        c = skip_parens(ast, c);
        let k = exprs.get(c).kind();
        if k == ExprKind::ImplicitCastExpr {
            c = exprs.operand(c);
            continue;
        }
        if !matches!(k, ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            return false;
        }
        let mut d = Some(exprs.decl(c));
        for _ in 0..4096 {
            let Some(x) = d else { break };
            if ast
                .lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(x))
                .any(|a| matches!(ast.attrs.get(a).kind, AttrKind::PURE | AttrKind::CONST))
            {
                return true;
            }
            d = ast.prev(x);
        }
        return false;
    }
    false
}

/// Whether evaluating `e` would modify an object, allocate, or throw on
/// its own (clang `Expr::HasSideEffects` without its treatment of calls:
/// a called function is evaluated, and a failure in it makes the result
/// 0). `__builtin_constant_p` of such an argument is 0 without evaluating it.
fn has_side_effects(ast: &Ast, e: ExprId) -> bool {
    side_effects_outside(ast, e, &[])
}

/// [`has_side_effects`] outside the subexpressions `skip`.
fn side_effects_outside(ast: &Ast, e: ExprId, skip: &[ExprId]) -> bool {
    let exprs = &ast.exprs;
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        if skip.contains(&x) {
            continue;
        }
        match exprs.get(x).kind() {
            ExprKind::UnaryOperator if exprs.unary_op(x).is_inc_dec() => return true,
            ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator
                if exprs.binary_op(x).is_assignment() =>
            {
                return true;
            }
            ExprKind::CXXNewExpr
            | ExprKind::CXXDeleteExpr
            | ExprKind::AtomicExpr
            | ExprKind::VAArgExpr
            | ExprKind::CXXThrowExpr
            | ExprKind::CoawaitExpr
            | ExprKind::CoyieldExpr => return true,
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::LambdaExpr
            | ExprKind::RequiresExpr => {}
            _ => exprs.children_into(x, &ast.lists, &mut stack),
        }
    }
    false
}

/// `StringRef::getAsInteger(0, APInt)`: an unsigned integer whose radix
/// is sensed from its prefix (`0x`, `0b`, `0o`, a leading `0`), modulo
/// 2^128 (only the low bits of a NaN payload are kept).
fn parse_auto_radix(s: &[u8]) -> Option<u128> {
    let lower = |i: usize| s.get(i).map(u8::to_ascii_lowercase);
    let (radix, digits) = match (s.first(), lower(1)) {
        (Some(b'0'), Some(b'x')) => (16, &s[2..]),
        (Some(b'0'), Some(b'b')) => (2, &s[2..]),
        (Some(b'0'), Some(b'o')) if s[1] == b'o' => (8, &s[2..]),
        (Some(b'0'), Some(c)) if c.is_ascii_digit() => (8, &s[1..]),
        _ => (10, s),
    };
    if digits.is_empty() {
        return None;
    }
    digits.iter().try_fold(0u128, |v, &c| {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'z' => c - b'a' + 10,
            b'A'..=b'Z' => c - b'A' + 10,
            _ => return None,
        };
        (u32::from(d) < radix)
            .then(|| v.wrapping_mul(u128::from(radix)).wrapping_add(u128::from(d)))
    })
}

/// clang's `TryEvaluateBuiltinNaN`: the argument must be a string literal
/// (an empty one is a zero payload).
fn nan_bits(ast: &Ast, mut arg: ExprId, kind: FloatKind, signaling: bool) -> Option<u128> {
    loop {
        arg = skip_parens(ast, arg);
        if !ast.exprs.get(arg).kind().is_cast() {
            break;
        }
        arg = ast.exprs.operand(arg);
    }
    if ast.exprs.get(arg).kind() != ExprKind::StringLiteral {
        return None;
    }
    let s = ast.strings.bytes(ast.exprs.string_lit(arg));
    let fill = if s.is_empty() { 0 } else { parse_auto_radix(s)? };
    Some(softfloat::make_nan(kind, fill, signaling))
}

/// The `bi::FP_MATH` operation of the P0533 `<cmath>` builtin `b`
/// (WP-365).
fn math_op(b: u32) -> Option<u32> {
    Some(match id::p0533_base(b)? {
        "ceil" => bi::CEIL,
        "floor" => bi::FLOOR,
        "trunc" => bi::TRUNC,
        "round" => bi::ROUND,
        "roundeven" => bi::ROUNDEVEN,
        "rint" | "nearbyint" => bi::RINT,
        "lround" | "llround" => bi::LROUND,
        "lrint" | "llrint" => bi::LRINT,
        "fmod" => bi::FMOD,
        "remainder" => bi::REMAINDER,
        "remquo" => bi::REMQUO,
        "fdim" => bi::FDIM,
        "fma" => bi::FMA,
        "frexp" => bi::FREXP,
        "ldexp" | "scalbn" | "scalbln" => bi::LDEXP,
        "ilogb" => bi::ILOGB,
        "logb" => bi::LOGB,
        "modf" => bi::MODF,
        "sqrt" => bi::SQRT,
        _ => bi::NEXTAFTER,
    })
}

/// The `bi::INT_OP` operation of the GCC integer builtin `b` (WP-150).
fn int_op(b: u32) -> Option<u32> {
    Some(match b {
        id::FFS | id::FFSL | id::FFSLL => bi::FFS,
        id::PARITY | id::PARITYL | id::PARITYLL => bi::PARITY,
        id::CLRSB | id::CLRSBL | id::CLRSBLL => bi::CLRSB,
        id::BITREVERSE8 | id::BITREVERSE16 | id::BITREVERSE32 | id::BITREVERSE64 => bi::BITREVERSE,
        id::ROTATELEFT8 | id::ROTATELEFT16 | id::ROTATELEFT32 | id::ROTATELEFT64 => bi::ROTL,
        id::ROTATERIGHT8 | id::ROTATERIGHT16 | id::ROTATERIGHT32 | id::ROTATERIGHT64 => bi::ROTR,
        _ => return None,
    })
}

impl Compiler<'_> {
    fn int_width(&mut self, e: ExprId) -> u32 {
        let t = self.ety(e);
        match self.kind(t) {
            TyKind::Int { width, .. } => u32::from(width),
            TyKind::Bool => 1,
            _ => 64,
        }
    }

    /// The width and signedness of integer (`bool`: 1, unsigned) operand
    /// `e`.
    fn int_ws(&mut self, e: ExprId) -> (u32, bool) {
        let t = self.ety(e);
        match self.kind(t) {
            TyKind::Int { width, signed } => (u32::from(width), signed),
            _ => (1, false),
        }
    }

    fn float_kind(&mut self, e: ExprId) -> Option<FloatKind> {
        let t = self.ety(e);
        match self.kind(t) {
            TyKind::Float(k) => Some(k),
            _ => None,
        }
    }

    /// The `t` operand of the object pointer argument `e`.
    fn designator(&mut self, e: ExprId) -> u32 {
        match designator_type(self.ast(), e) {
            Some(q) => self.ty(q).0,
            None => bi::DYN,
        }
    }

    /// The verdict of the union member accesses designating the object
    /// that the `is_within_lifetime` pointer argument `x` points to
    /// (WP-418; clang reads its `LValue` designator, which an Oven pointer
    /// does not carry): for `&a.b.c` (through pointer casts) rooted at a
    /// variable or `this`, each access `m` of a union member is checked
    /// (`bi::UNION_LIVE`) to name the active member, from register
    /// `init` (else true). `None` when `x` designates no union member so.
    /// The accesses have no side effects; the argument was evaluated.
    pub(super) fn union_designators(&mut self, x: ExprId, init: Option<u32>) -> Option<u32> {
        use kiln_ast::expr::{ExprFlags, UnaryOp};
        let ast = self.ast();
        let a = strip_pointer_casts(ast, x);
        if ast.exprs.get(a).kind() != ExprKind::UnaryOperator
            || ast.exprs.unary_op(a) != UnaryOp::AddrOf
        {
            return None;
        }
        let mut steps = Vec::new();
        let mut m = skip_parens(ast, ast.exprs.operand(a));
        loop {
            let n = ast.exprs.get(m);
            match n.kind() {
                ExprKind::MemberExpr
                    if ast.decls.get(ast.exprs.decl(m)).kind()
                        == kiln_ast::decl::DeclKind::Field =>
                {
                    let base = skip_parens(ast, ast.exprs.operand(m));
                    steps.push(m);
                    if n.has(ExprFlags::ARROW) {
                        if ast.exprs.get(base).kind() != ExprKind::CXXThisExpr {
                            return None;
                        }
                        break;
                    }
                    m = base;
                }
                ExprKind::DeclRefExpr
                    if ast.decls.get(ast.exprs.decl(m)).kind() == kiln_ast::decl::DeclKind::Var =>
                {
                    break;
                }
                _ => return None,
            }
        }
        let mut ok = init;
        for m in steps.into_iter().rev() {
            let ast = self.ast();
            let base = ast.exprs.operand(m);
            let fd = ast.exprs.decl(m);
            let arrow = ast.exprs.get(m).has(ExprFlags::ARROW);
            let bq = ast.exprs.get(base).ty();
            let bt = self.ty(bq);
            let ut = match self.kind(bt) {
                TyKind::Ptr(p) if arrow => p,
                _ => bt,
            };
            if !matches!(self.kind(ut), TyKind::Record { union: true, .. }) {
                continue;
            }
            let Some(idx) = self.o.types.fields(ut).iter().position(|f| f.decl == fd) else {
                continue;
            };
            let p = if arrow { self.rv(base) } else { self.lv_mem(base, super::Access::Addr) };
            let prev = match ok {
                Some(r) => r,
                None => {
                    let one = self.reg();
                    self.konst(one, 1);
                    one
                }
            };
            let r = self.reg();
            let loc = self.begin(m);
            let k = bi::UNION_LIVE | (idx as u32) << 8;
            self.emit_site(Op::Builtin, k, &[r, p, prev, prev, ut.0, 0], m, loc);
            ok = Some(r);
        }
        ok.filter(|&r| Some(r) != init)
    }

    /// clang's `FoldConstant` of a conditional whose condition (through
    /// parentheses and casts) is a `__builtin_constant_p` call, a GNU
    /// extension (GCC PR38377): the whole conditional is evaluated as a
    /// fold (`RunState::fold`), so a cast a constant expression may not
    /// perform ([expr.const]/5.15) does not fail it. Emits the `Op::Fold`
    /// that enters the fold and returns the register saving the previous
    /// mode, for [`Self::bcp_fold_exit`].
    pub(super) fn bcp_fold_enter(&mut self, cond: ExprId) -> Option<u32> {
        let ast = self.ast();
        let mut c = skip_parens(ast, cond);
        while ast.exprs.get(c).kind().is_cast() {
            c = skip_parens(ast, ast.exprs.operand(c));
        }
        if ast.exprs.get(c).kind() != ExprKind::CallExpr {
            return None;
        }
        let mut f = skip_parens(ast, ast.exprs.callee(c));
        while ast.exprs.get(f).kind().is_cast() {
            f = skip_parens(ast, ast.exprs.operand(f));
        }
        if ast.exprs.get(f).kind() != ExprKind::DeclRefExpr
            || ast.decls.builtin_id(ast.exprs.decl(f)) != Some(id::CONSTANT_P)
        {
            return None;
        }
        let r = self.reg();
        self.emit(Op::Fold, 1, &[r]);
        Some(r)
    }

    /// Leaves the fold [`Self::bcp_fold_enter`] entered.
    pub(super) fn bcp_fold_exit(&mut self, saved: Option<u32>) {
        if let Some(r) = saved {
            self.emit(Op::Fold, 0, &[r]);
        }
    }

    /// clang `tryEvaluateBuiltinObjectSize` (WP-292) of pointer `p` for
    /// the type in register `b`, into `dst` (`site`: the reporting
    /// expression): the pointer without its pointer casts, evaluated
    /// speculatively. clang fails any modification of an object from
    /// before the trial and ignores a failing comma operand: the comma
    /// left operands of `skipped_comma_lhs` are skipped (any depth,
    /// WP-302), and a remaining side effect fails the trial. Otherwise the
    /// pointer runs as a trial. With `default` (the builtin's
    /// `ExprConstant` case), a failure gives the default when the operand
    /// has side effects (calls included, clang `HasSideEffects`), else an
    /// invalid subexpression at the call; without it (codegen's
    /// `tryEvaluateObjectSize` of a `pass_object_size` argument, WP-808),
    /// any failure fails. First, a pointer into the result of an
    /// `alloc_size` call folds to its size ([`Compiler::alloc_size_object`];
    /// `kind`: the type, when it is a constant).
    pub(super) fn object_size(
        &mut self,
        site: ExprId,
        p: ExprId,
        b: u32,
        dst: u32,
        begin: Loc,
        default: bool,
        kind: Option<u32>,
    ) {
        if let Some(n) = self.alloc_size_object(p, kind) {
            self.konst(dst, u128::from(n));
            return;
        }
        let ast = self.ast();
        let p = strip_pointer_casts(ast, p);
        let des = designator_type(ast, p);
        let side = default && (has_side_effects(ast, p) || has_calls(ast, p));
        let skip = skipped_comma_lhs(ast, p);
        let still = side_effects_outside(ast, p, &skip);
        let ty = des.map_or(bi::DYN, |q| self.ty(q).0);
        let (a, ok) = if still {
            let ok = self.reg();
            self.konst(ok, 0);
            (b, ok)
        } else {
            let ok = self.reg();
            let pc = self.emit(Op::Speculate, 0, &[ok, 0, 0]);
            let saved = std::mem::replace(&mut self.skip_effects, skip);
            let v = self.rv(p);
            self.skip_effects = saved;
            self.emit(Op::Ret, 1, &[v]);
            let end = self.here();
            self.patch(pc + 2, v);
            self.patch(pc + 3, end);
            (v, ok)
        };
        let sub = bi::OBJECT_SIZE | (u32::from(side) * bi::SIDE) << 8;
        self.emit_site(Op::Builtin, sub, &[dst, a, b, ok, ty, 0], site, begin);
    }

    /// The object size of pointer `p` whose object is the result of a call
    /// of an `alloc_size` function (clang `tryEvaluateBuiltinObjectSize`
    /// with an invalid base, WP-1073): `p` is such a call with constant
    /// size arguments (`getBytesReturnedByAllocSizeCall`: their product,
    /// failing on overflow), an lvalue-to-rvalue read of a local `const`
    /// pointer variable initialized by one (`evaluateLValueAsAllocSize`,
    /// through a full-expression and one cast), or either plus or minus a
    /// constant number of elements. The result is the allocated size less
    /// the offset, 0 past the end or before the start, for every type (the
    /// base is an unsized array, so it refers to the complete object).
    /// `None` leaves `p` to the trial.
    fn alloc_size_object(&mut self, p: ExprId, kind: Option<u32>) -> Option<u64> {
        let p = strip_pointer_casts(self.ast(), p);
        let (size, off, valid) = self.alloc_base(p, 0)?;
        // clang `determineEndOffset`: type 3 wants a lower bound, which an
        // invalid designator does not give.
        if !valid && kind? == 3 {
            return None;
        }
        Some(if off < 0 { 0 } else { size.saturating_sub(u64::try_from(off).ok()?) })
    }

    /// The allocated size, byte offset and designator validity of
    /// [`Compiler::alloc_size_object`]. A cast to a non-`void` pointer
    /// inside `p` invalidates the designator (clang's `CK_BitCast` of a
    /// pointer with an invalid base).
    fn alloc_base(&mut self, p: ExprId, depth: u32) -> Option<(u64, i128, bool)> {
        use kiln_ast::expr::{CastKind, ValueCategory};
        if depth > 32 {
            return None;
        }
        let ast = self.ast();
        let exprs = &ast.exprs;
        let mut p = skip_parens(ast, p);
        let mut valid = true;
        while exprs.get(p).kind().is_cast()
            && matches!(exprs.cast_kind(p), CastKind::NoOp | CastKind::BitCast)
        {
            let sub = exprs.operand(p);
            let s = exprs.get(sub);
            if ast.types.pointee(s.ty()).is_none() || s.value_category() != ValueCategory::PRValue {
                break;
            }
            if exprs.cast_kind(p) == CastKind::BitCast
                && !ast.types.pointee(exprs.get(p).ty()).is_some_and(|q| ast.types.is_void(q))
            {
                valid = false;
            }
            p = skip_parens(ast, sub);
        }
        match exprs.get(p).kind() {
            ExprKind::CallExpr => Some((self.alloc_call_size(p)?, 0, valid)),
            ExprKind::BinaryOperator
                if matches!(exprs.binary_op(p), BinaryOp::Add | BinaryOp::Sub) =>
            {
                let op = exprs.binary_op(p);
                let [l, r] = exprs.operands(p);
                let (ptr, n) = if ast.types.pointee(exprs.get(l).ty()).is_some() {
                    (l, r)
                } else if op == BinaryOp::Add {
                    (r, l)
                } else {
                    return None;
                };
                let elem = ast.types.pointee(exprs.get(p).ty())?;
                if ast.types.is_void(elem) || is_vla(ast, elem) {
                    return None;
                }
                let k = self.alloc_size_int(n, depth + 1)?;
                let t = self.ty(elem);
                let w = i128::from(self.o.types.size(t));
                if w == 0 {
                    return None;
                }
                let (size, off, inner) = self.alloc_base(ptr, depth + 1)?;
                let d = k.checked_mul(w)?;
                let off = if op == BinaryOp::Add { off.checked_add(d)? } else { off.checked_sub(d)? };
                Some((size, off, valid && inner))
            }
            ExprKind::ImplicitCastExpr if exprs.cast_kind(p) == CastKind::LValueToRValue => {
                let r = skip_parens(ast, exprs.operand(p));
                if exprs.get(r).kind() != ExprKind::DeclRefExpr {
                    return None;
                }
                let d = exprs.decl(r);
                let n = ast.decls.get(d);
                let local = n.kind() == kiln_ast::decl::DeclKind::Var
                    && n.ctx().is_some_and(|c| ast.decls.get(c).kind().is_function());
                let v = ast.decls.var(d)?;
                let ty = n.ty()?;
                if !local || !ast.types.effective_quals(ty).is_const() {
                    return None;
                }
                // clang `tryUnwrapAllocSizeCall`.
                let mut e = skip_parens(ast, v.init?);
                if exprs.get(e).kind() == ExprKind::ExprWithCleanups {
                    e = skip_parens(ast, exprs.operand(e));
                }
                if exprs.get(e).kind().is_cast() {
                    e = skip_parens(ast, exprs.operand(e));
                }
                if exprs.get(e).kind() != ExprKind::CallExpr {
                    return None;
                }
                Some((self.alloc_call_size(e)?, 0, valid))
            }
            _ => None,
        }
    }

    /// The bytes a call `e` of a function with `alloc_size` returns, its
    /// size arguments constant (clang `getBytesReturnedByAllocSizeCall`).
    fn alloc_call_size(&mut self, e: ExprId) -> Option<u64> {
        use kiln_ast::attr::AttrKind;
        let ast = self.ast();
        let exprs = &ast.exprs;
        let mut c = exprs.callee(e);
        loop {
            c = skip_parens(ast, c);
            if exprs.get(c).kind() != ExprKind::ImplicitCastExpr {
                break;
            }
            c = exprs.operand(c);
        }
        if exprs.get(c).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = exprs.decl(c);
        if ast.decls.get(d).kind() != kiln_ast::decl::DeclKind::Function {
            return None;
        }
        let attr = ast.redecls_of(d).find_map(|r| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .map(|a| *ast.attrs.get(a))
                .find(|n| n.kind == AttrKind::ALLOC_SIZE)
        })?;
        let index = |raw: u32| {
            let x = <ExprId as kiln_ast::ids::RawId>::from_raw(raw)?;
            (exprs.get(x).kind() == ExprKind::IntegerLiteral)
                .then(|| exprs.int_value(x))
                .flatten()
                .and_then(|v| usize::try_from(v).ok()?.checked_sub(1))
        };
        let args: Vec<ExprId> =
            ast.lists.get(exprs.call_args(e)).iter().map(|&x| ExprId::new(x)).collect();
        let first = *args.get(index(attr.a)?)?;
        let second = if attr.b == 0 { None } else { Some(*args.get(index(attr.b)?)?) };
        let size_t = QualType::unqual(ast.types.target().size_t);
        let st = self.ty(size_t);
        let size_bits = 8 * self.o.types.size(st);
        let mut n = self.alloc_size_int(first, 0)?;
        if let Some(s) = second {
            n = n.checked_mul(self.alloc_size_int(s, 0)?)?;
        }
        // The arguments are `size_t`; a product past it fails.
        let n = u64::try_from(n).ok()?;
        (size_bits >= 64 || n >> size_bits == 0).then_some(n)
    }

    /// The value of the integer constant `e` of an `alloc_size` argument
    /// or an offset: literals, integral conversions, `sizeof(type)`,
    /// `+ - *` and `constexpr` variables; `None` for anything else (the
    /// pointer is then left to the trial). Negative values fail.
    fn alloc_size_int(&mut self, e: ExprId, depth: u32) -> Option<i128> {
        use kiln_ast::expr::{CastKind, TraitKind};
        if depth > 32 {
            return None;
        }
        let ast = self.ast();
        let exprs = &ast.exprs;
        let e = skip_parens(ast, e);
        let n = exprs.get(e);
        let v = match n.kind() {
            ExprKind::IntegerLiteral => i128::from(exprs.int_value(e)?),
            k if k.is_cast() => {
                if !matches!(
                    exprs.cast_kind(e),
                    CastKind::IntegralCast | CastKind::NoOp | CastKind::LValueToRValue
                ) || !ast.types.is_integral(n.ty())
                {
                    return None;
                }
                self.alloc_size_int(exprs.operand(e), depth + 1)?
            }
            ExprKind::DeclRefExpr => {
                let d = exprs.decl(e);
                let v = ast.decls.var(d)?;
                if !v.spec.is_constexpr() {
                    return None;
                }
                self.alloc_size_int(v.init?, depth + 1)?
            }
            ExprKind::UnaryExprOrTypeTraitExpr if exprs.trait_kind(e) == TraitKind::SizeOf => {
                let q = exprs.trait_type_arg(e)?;
                if is_vla(ast, q) || ast.types.is_reference(q) || ast.types.is_void(q) {
                    return None;
                }
                let t = self.ty(q);
                i128::from(self.o.types.size(t))
            }
            ExprKind::BinaryOperator => {
                let op = exprs.binary_op(e);
                let [l, r] = exprs.operands(e);
                let (a, b) = (self.alloc_size_int(l, depth + 1)?, self.alloc_size_int(r, depth + 1)?);
                match op {
                    BinaryOp::Add => a.checked_add(b)?,
                    BinaryOp::Sub => a.checked_sub(b)?,
                    BinaryOp::Mul => a.checked_mul(b)?,
                    _ => return None,
                }
            }
            _ => return None,
        };
        (0..=i128::from(u64::MAX)).contains(&v).then_some(v)
    }

    /// Lowers call `e` of builtin `bid` into a fresh register; `None` for
    /// a builtin evaluated as an ordinary call.
    pub(super) fn builtin_call(&mut self, e: ExprId, bid: u32, args: &[ExprId]) -> Option<u32> {
        let begin = self.begin(e);
        let dst = self.reg();
        let arg = |i: usize| args.get(i).copied();
        // `Builtin` with kind `k`, `sub`, registers and types.
        let op = |s: &mut Self, k: u32, sub: u32, r: [u32; 3], t: [u32; 2], loc| {
            s.emit_site(Op::Builtin, k | sub << 8, &[dst, r[0], r[1], r[2], t[0], t[1]], e, loc);
        };
        // WP-282: a `__float128` builtin with a `long double` twin
        // evaluates as the twin (the operand types pick the format).
        let bid = id::f128_twin(bid).unwrap_or(bid);
        match bid {
            id::CONSTANT_P => {
                let x = arg(0)?;
                let k = self.ety(x);
                let ptr = matches!(self.kind(k), TyKind::Ptr(_));
                let scalar = ptr
                    || matches!(
                        self.kind(k),
                        TyKind::Bool | TyKind::Int { .. } | TyKind::Float(_) | TyKind::NullPtr
                    );
                if !scalar || has_side_effects(self.b.ast(), x) {
                    // clang `EvaluateBuiltinConstantP`: any other type, or
                    // an argument with side effects, is not a constant.
                    self.konst(dst, 0);
                } else {
                    // The argument is evaluated speculatively in this
                    // frame's registers: the region ends in a `Ret` of
                    // its value, after which `Speculate` resumes at `end`.
                    let pc = self.emit(Op::Speculate, u32::from(ptr) | 2, &[dst, 0, 0]);
                    let v = self.rv(x);
                    self.emit(Op::Ret, 1, &[v]);
                    let end = self.here();
                    self.patch(pc + 2, v);
                    self.patch(pc + 3, end);
                }
            }
            // WP-419: clang `EvaluateBuiltinClassifyType`: the operand's
            // GCC type class; the operand is not evaluated.
            id::CLASSIFY_TYPE => {
                let ast = self.ast();
                let v = arg(0).map_or(-1, |x| ast.gcc_type_class(ast.exprs.get(x).ty()));
                self.konst(dst, i128::from(v) as u128);
            }
            id::CLZ
            | id::CLZL
            | id::CLZLL
            | id::CLZG
            | id::CTZ
            | id::CTZL
            | id::CTZLL
            | id::CTZG => {
                let x = arg(0)?;
                let w = self.int_width(x);
                let a = self.rv(x);
                let (fb, has) = match arg(1) {
                    Some(f) => (self.rv(f), 1),
                    None => (dst, 0),
                };
                let k = if matches!(bid, id::CLZ | id::CLZL | id::CLZLL | id::CLZG) {
                    bi::CLZ
                } else {
                    bi::CTZ
                };
                op(self, k, w | has << 8, [a, fb, dst], [0, 0], begin);
            }
            id::POPCOUNT
            | id::POPCOUNTL
            | id::POPCOUNTLL
            | id::POPCOUNTG
            | id::BSWAP16
            | id::BSWAP32
            | id::BSWAP64 => {
                let x = arg(0)?;
                let w = self.int_width(x);
                let a = self.rv(x);
                let k = if matches!(bid, id::BSWAP16 | id::BSWAP32 | id::BSWAP64) {
                    bi::BSWAP
                } else {
                    bi::POPCOUNT
                };
                op(self, k, w, [a, dst, dst], [0, 0], begin);
            }
            id::ISFINITE
            | id::ISINF
            | id::ISNAN
            | id::ISNORMAL
            | id::ISSUBNORMAL
            | id::ISZERO
            | id::ISSIGNALING
            | id::ISINF_SIGN => {
                let x = arg(0)?;
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let a = self.rv(x);
                let class = match bid {
                    id::ISFINITE => bi::FINITE,
                    id::ISINF => bi::INF,
                    id::ISNAN => bi::NAN,
                    id::ISSUBNORMAL => bi::SUBNORMAL,
                    id::ISZERO => bi::ZERO,
                    id::ISSIGNALING => bi::SIGNALING,
                    id::ISINF_SIGN => {
                        let sub = float_code(fk) | bi::ISINF_SIGN << 4;
                        op(self, bi::FP_OP, sub, [a, dst, dst], [0, 0], begin);
                        return Some(self.finish_builtin(dst));
                    }
                    _ => bi::NORMAL,
                };
                op(self, bi::FP_CLASS, float_code(fk) | class << 4, [a, dst, dst], [0, 0], begin);
            }
            // clang: the mask (an integer constant) first, then the value.
            id::ISFPCLASS => {
                let (x, m) = (arg(0)?, arg(1)?);
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let b = self.rv(m);
                let a = self.rv(x);
                op(self, bi::FP_CLASS, float_code(fk) | bi::MASK << 4, [a, b, dst], [0, 0], begin);
            }
            id::FPCLASSIFY => {
                // clang: the operand, then only the argument its class
                // selects (nan, infinite, normal, subnormal, zero).
                let (Some(x), true) = (arg(5), args.len() == 6) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let a = self.rv(x);
                let mut ends = Vec::new();
                for (i, class) in [(0, bi::NAN), (1, bi::INF), (2, bi::NORMAL), (3, bi::SUBNORMAL)]
                {
                    op(
                        self,
                        bi::FP_CLASS,
                        float_code(fk) | class << 4,
                        [a, dst, dst],
                        [0, 0],
                        begin,
                    );
                    let j = self.jmp_if(dst, false);
                    self.rv_into(args[i], dst);
                    ends.push(self.jmp());
                    self.patch_here(j);
                }
                self.rv_into(args[4], dst);
                for j in ends {
                    self.patch_here(j);
                }
            }
            id::FABS
            | id::FABSF
            | id::FABSL
            | id::COPYSIGN
            | id::COPYSIGNF
            | id::COPYSIGNL
            | id::COPYSIGNF16
            | id::FMAX
            | id::FMAXF
            | id::FMAXL
            | id::FMIN
            | id::FMINF
            | id::FMINL
            | id::FMAXIMUM_NUMF16
            | id::FMINIMUM_NUMF16
            | id::FMAXIMUM_NUM
            | id::FMAXIMUM_NUMF
            | id::FMAXIMUM_NUML
            | id::FMINIMUM_NUM
            | id::FMINIMUM_NUMF
            | id::FMINIMUM_NUML => {
                let Some(fk) = self.float_kind(e) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                // WP-403: clang 22 folds `fmax`/`fmin` with LLVM 22's
                // `maxnum`/`minnum`, which quiet a signaling NaN operand.
                let c22 = self.o.clang_compat >= 22;
                let k = match bid {
                    id::FABS | id::FABSF | id::FABSL => bi::FABS,
                    id::COPYSIGN | id::COPYSIGNF | id::COPYSIGNL | id::COPYSIGNF16 => {
                        bi::COPYSIGN
                    }
                    id::FMAX | id::FMAXF | id::FMAXL if c22 => bi::FMAX_22,
                    id::FMAX | id::FMAXF | id::FMAXL => bi::FMAX,
                    id::FMAXIMUM_NUMF16
                    | id::FMAXIMUM_NUM
                    | id::FMAXIMUM_NUMF
                    | id::FMAXIMUM_NUML => bi::FMAXIMUM_NUM,
                    id::FMINIMUM_NUMF16
                    | id::FMINIMUM_NUM
                    | id::FMINIMUM_NUMF
                    | id::FMINIMUM_NUML => bi::FMINIMUM_NUM,
                    _ if c22 => bi::FMIN_22,
                    _ => bi::FMIN,
                };
                let a = self.rv(arg(0)?);
                let b = match arg(1) {
                    Some(y) => self.rv(y),
                    None => a,
                };
                op(self, bi::FP_OP, float_code(fk) | k << 4, [a, b, dst], [0, 0], begin);
            }
            id::INF | id::INFF | id::INFL | id::HUGE_VAL | id::HUGE_VALF | id::HUGE_VALL => {
                let fk = self.float_kind(e)?;
                self.konst(dst, softfloat::inf(fk));
            }
            id::NAN | id::NANF | id::NANL | id::NANS | id::NANSF | id::NANSL => {
                let fk = self.float_kind(e)?;
                let signaling = matches!(bid, id::NANS | id::NANSF | id::NANSL);
                match arg(0).and_then(|x| nan_bits(self.ast(), x, fk, signaling)) {
                    Some(bits) => self.konst(dst, bits),
                    None => self.trap_loc(begin, INVALID.into()),
                }
            }
            id::LAUNDER => self.rv_into(arg(0)?, dst),
            id::IS_STRING_LITERAL => {
                let p = self.rv(arg(0)?);
                op(self, bi::STRING_LIT, 0, [p, p, p], [0, 0], begin);
            }
            // clang: the value of the first operand (the expected value is
            // not evaluated).
            id::EXPECT | id::EXPECT_WITH_PROBABILITY => self.rv_into(arg(0)?, dst),
            // clang: the assumption is not evaluated.
            id::ASSUME => self.konst(dst, 0),
            // WP-757: a P1494R5 checkpoint has no effect on constant
            // evaluation, which has no undefined behaviour to bound (GCC
            // also evaluates `__builtin_observable_checkpoint`; the
            // library's `std::observable_checkpoint` is not constexpr).
            id::OBSERVABLE_CHECKPOINT => self.konst(dst, 0),
            // WP-773: P3726R2 `std::start_lifetime` (`Oven::start_lifetime`).
            id::START_LIFETIME => {
                let x = arg(0)?;
                let t = self.designator(x);
                let p = self.rv(x);
                op(self, bi::START_LIFETIME, 0, [p, p, p], [t, 0], begin);
            }
            // WP-774: GCC `__builtin_clear_padding` (a GCC-only kiln
            // builtin; GCC itself rejects it in a constant expression):
            // the pointer is checked, the padding left as it is
            // (`Oven::clear_padding`).
            id::CLEAR_PADDING => {
                let x = arg(0)?;
                let t = self.designator(x);
                let p = self.rv(x);
                op(self, bi::CLEAR_PADDING, 0, [p, p, p], [t, 0], begin);
            }
            // WP-292: the pointer, checked against the alignment (and
            // offset); a failure is noted at the pointer operand.
            id::ASSUME_ALIGNED => {
                let p = arg(0)?;
                let r = [self.rv(p), self.rv(arg(1)?), arg(2).map_or(dst, |o| self.rv(o))];
                let at = self.ast().exprs.get(p).loc();
                op(self, bi::ASSUME_ALIGNED, u32::from(arg(2).is_some()), r, [0, 0], at);
            }
            // WP-418: the pointer's own checks, then its designator's
            // unions (spec/oven.md "Builtins").
            id::IS_WITHIN_LIFETIME => {
                let x = arg(0)?;
                let t = self.designator(x);
                let p = self.rv(x);
                let ok = match self.union_designators(x, None) {
                    Some(ok) => ok,
                    None => {
                        let one = self.reg();
                        self.konst(one, 1);
                        one
                    }
                };
                op(self, bi::LIFETIME, 0, [p, ok, p], [t, 0], begin);
            }
            id::OBJECT_SIZE | id::DYNAMIC_OBJECT_SIZE => {
                let (p, t) = (arg(0)?, arg(1)?);
                let b = self.rv(t);
                let kind = self.alloc_size_int(t, 0).and_then(|k| u32::try_from(k).ok());
                self.object_size(e, p, b, dst, begin, true, kind);
            }
            // As `::operator new`/`delete` (spec/oven.md "Transient
            // allocation").
            id::OPERATOR_NEW => return Some(self.global_alloc_call(e, true, args)),
            id::OPERATOR_DELETE => return Some(self.global_alloc_call(e, false, args)),
            // Not evaluated by clang 19: the call is not a constant
            // expression, its arguments unevaluated.
            id::UNREACHABLE
            | id::TRAP
            | id::ABORT
            | id::STRRCHR
            | id::STRPBRK
            | id::STRSTR
            | id::MEMSET
            | id::MEMPCPY
            // WP-270: clang evaluates neither `alloca` nor the
            // `printf` family.
            | id::ALLOCA
            | id::ALLOCA_WITH_ALIGN
            | id::VSNPRINTF
            | id::SNPRINTF
            | id::SPRINTF
            | id::PRINTF
            | id::VPRINTF
            | id::VSPRINTF
            | id::SCANF
            | id::SSCANF
            | id::VSCANF
            | id::VSSCANF
            // clang evaluates no `va_list` operation.
            | id::VA_START
            | id::VA_END
            | id::VA_COPY
            // WP-282: nor `powi`, nor `verbose_trap`.
            | id::POWI
            | id::POWIF
            | id::POWIL
            | id::VERBOSE_TRAP
            // WP-432: nor the frame builtins.
            | id::DEBUGTRAP
            | id::FRAME_ADDRESS
            | id::RETURN_ADDRESS
            // WP-782: nor the thread pointer.
            | id::THREAD_POINTER
            // WP-1057: nor the cycle counter.
            | id::READCYCLECOUNTER
            // WP-610: nor `__builtin_prefetch`.
            | id::PREFETCH
            // WP-630: nor the nontemporal accesses and
            // `__builtin_nondeterministic_value`.
            | id::NONTEMPORAL_STORE
            | id::NONTEMPORAL_LOAD
            | id::NONDETERMINISTIC_VALUE => self.trap_loc(begin, INVALID.into()),
            // WP-630: the x86 builtins clang evaluates; any other is not a
            // constant expression.
            _ if id::x86::is_x86(bid) => return Some(self.x86_builtin_call(e, bid, args, dst)),
            // WP-365: the P0533 `<cmath>` set, exact in every format.
            id::SIGNBIT
            | id::ISGREATER
            | id::ISGREATEREQUAL
            | id::ISLESS
            | id::ISLESSEQUAL
            | id::ISLESSGREATER
            | id::ISUNORDERED => {
                let x = arg(0)?;
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let (m, c) = match bid {
                    id::SIGNBIT => (bi::SIGNBIT, 0),
                    id::ISGREATER => (bi::CMP, cmp::GT),
                    id::ISGREATEREQUAL => (bi::CMP, cmp::GE),
                    id::ISLESS => (bi::CMP, cmp::LT),
                    id::ISLESSEQUAL => (bi::CMP, cmp::LE),
                    id::ISLESSGREATER => (bi::CMP, cmp::NE),
                    _ => (bi::CMP, bi::UNORDERED),
                };
                let a = self.rv(x);
                let b = match arg(1) {
                    Some(y) => self.rv(y),
                    None => a,
                };
                op(self, bi::FP_MATH, float_code(fk) | m << 4, [a, b, dst], [c, 0], begin);
            }
            id::ABS | id::LABS | id::LLABS => {
                let x = arg(0)?;
                let w = self.int_width(x);
                let a = self.rv(x);
                op(self, bi::INT_OP, w | bi::IABS << 8, [a, dst, dst], [0, 0], begin);
            }
            _ if math_op(bid).is_some() => {
                let m = math_op(bid)?;
                let x = arg(0)?;
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let sub = float_code(fk) | m << 4;
                let a = self.rv(x);
                // The second operand (`fma`: and third); the pointer
                // operand of `frexp`/`modf`/`remquo`.
                let (n, out) = match m {
                    bi::FREXP | bi::MODF => (1, arg(1)),
                    bi::REMQUO => (2, arg(2)),
                    bi::FMA => (3, None),
                    bi::FMOD
                    | bi::REMAINDER
                    | bi::FDIM
                    | bi::LDEXP
                    | bi::NEXTAFTER => (2, None),
                    _ => (1, None),
                };
                let b = if n >= 2 { self.rv(arg(1)?) } else { a };
                let c = if n >= 3 { self.rv(arg(2)?) } else { a };
                let t1 = match m {
                    bi::LROUND | bi::LRINT => self.int_width(e),
                    bi::NEXTAFTER => match self.float_kind(arg(1)?) {
                        Some(k) => float_code(k),
                        None => float_code(fk),
                    },
                    _ => 0,
                };
                let p = out.map(|o| (self.rv(o), self.ast().exprs.get(o).ty()));
                op(self, bi::FP_MATH, sub, [a, b, c], [t1, 0], begin);
                if let Some((p, pq)) = p {
                    let rt = self.ty(self.ast().types.pointee(pq)?);
                    let v = self.reg();
                    self.emit_site(
                        Op::Builtin,
                        bi::FP_MATH | (sub | bi::SECOND) << 8,
                        &[v, a, b, c, t1, 0],
                        e,
                        begin,
                    );
                    self.emit_site(Op::Store, ak::ASSIGN, &[p, v, rt.0], e, begin);
                }
            }
            // The other libm functions and the rounding intrinsics.
            _ if id::is_libm(bid)
                || matches!(
                    bid,
                    id::CANONICALIZEF16
                        | id::CANONICALIZE
                        | id::CANONICALIZEF
                        | id::CANONICALIZEL
                ) =>
            {
                self.trap_loc(begin, INVALID.into());
            }
            // WP-171: clang constant-evaluates no coroutine builtin.
            _ if id::is_coro(bid) => self.trap_loc(begin, INVALID.into()),
            // WP-803: clang evaluates no `_FORTIFY_SOURCE` checking builtin.
            _ if id::is_chk(bid) => self.trap_loc(begin, INVALID.into()),
            id::STRLEN | id::WCSLEN => {
                let x = arg(0)?;
                let q = self.ast().exprs.get(x).ty();
                let ct = self.ast().types.pointee(q)?;
                let t = self.ty(ct).0;
                let a = self.rv(x);
                // clang reads through the argument (notes at its location).
                let loc = self.ast().exprs.get(x).loc();
                op(self, bi::STRLEN, 0, [a, dst, dst], [t, 0], loc);
            }
            id::MEMCMP
            | id::BCMP
            | id::WMEMCMP
            | id::STRCMP
            | id::STRNCMP
            | id::WCSCMP
            | id::WCSNCMP => {
                let (x, y) = (arg(0)?, arg(1)?);
                let mut sub = u32::from(matches!(bid, id::WMEMCMP | id::WCSCMP | id::WCSNCMP));
                if matches!(bid, id::STRCMP | id::STRNCMP | id::WCSCMP | id::WCSNCMP) {
                    sub |= bi::STR;
                }
                let n = if matches!(bid, id::STRCMP | id::WCSCMP) {
                    sub |= bi::UNBOUNDED;
                    None
                } else {
                    Some(arg(2)?)
                };
                let t = [self.designator(x), self.designator(y)];
                let (a, b) = (self.rv(x), self.rv(y));
                let c = n.map_or(dst, |n| self.rv(n));
                op(self, bi::MEMCMP, sub, [a, b, c], t, begin);
            }
            id::MEMCHR | id::CHAR_MEMCHR | id::WMEMCHR | id::STRCHR | id::WCSCHR => {
                let (x, v) = (arg(0)?, arg(1)?);
                let mut sub = u32::from(matches!(bid, id::WMEMCHR | id::WCSCHR));
                let n = if matches!(bid, id::STRCHR | id::WCSCHR) {
                    sub |= bi::STR | bi::UNBOUNDED;
                    None
                } else {
                    Some(arg(2)?)
                };
                let t = self.designator(x);
                let (a, b) = (self.rv(x), self.rv(v));
                let c = n.map_or(dst, |n| self.rv(n));
                op(self, bi::MEMCHR, sub, [a, b, c], [t, 0], begin);
            }
            // WP-154: clang's `IntExprEvaluator` lock-free folding.
            id::atomic::ALWAYS_LOCK_FREE
            | id::atomic::IS_LOCK_FREE
            | id::atomic::C11_IS_LOCK_FREE => {
                let size = arg(0)?;
                let max = u32::from(self.ast().types.target().max_atomic_inline_bytes);
                let (any, align, int) = if bid == id::atomic::C11_IS_LOCK_FREE {
                    (true, 0, None)
                } else {
                    let p = arg(1)?;
                    let (any, align) = self.lock_free_object(p);
                    // WP-726: clang 19 first folds the pointer to an
                    // integer (libstdc++'s `reinterpret_cast<void *>(
                    // -_S_alignment)`) and checks its alignment; only
                    // needed when the type's alignment may not suffice.
                    let int = if any || align >= max { None } else { self.lock_free_int(p) };
                    (any, align, int)
                };
                let a = self.rv(size);
                let b = int.map_or(dst, |x| self.rv(x));
                let sub = max
                    | u32::from(any) << 8
                    | u32::from(bid == id::atomic::ALWAYS_LOCK_FREE) << 9
                    | u32::from(int.is_some()) << 10;
                op(self, bi::LOCK_FREE, sub, [a, b, dst], [align, 0], begin);
            }
            id::MEMCPY | id::MEMMOVE | id::WMEMCPY | id::WMEMMOVE => {
                let (d, s, n) = (arg(0)?, arg(1)?, arg(2)?);
                let t = [self.designator(d), self.designator(s)];
                let r = [self.rv(d), self.rv(s), self.rv(n)];
                let mut sub = u32::from(matches!(bid, id::MEMMOVE | id::WMEMMOVE)) * bi::MOVE;
                if matches!(bid, id::WMEMCPY | id::WMEMMOVE) {
                    sub |= bi::WCHAR;
                }
                op(self, bi::MEMCPY, sub, r, t, begin);
            }
            // WP-150: the GCC integer builtins.
            _ if int_op(bid).is_some() => {
                let k = int_op(bid)?;
                let x = arg(0)?;
                let w = self.int_width(x);
                let a = self.rv(x);
                let b = match arg(1) {
                    Some(n) => self.rv(n),
                    None => dst,
                };
                op(self, bi::INT_OP, w | k << 8, [a, b, dst], [0, 0], begin);
            }
            // WP-270: checked arithmetic (clang `IntExprEvaluator`
            // `BI__builtin_*_overflow`): both operands, then the result
            // pointer, are evaluated; the wrapped value is stored through
            // the pointer and the overflow flag is the call's value.
            _ if id::checked_op(bid).is_some() => {
                let (k, _) = id::checked_op(bid)?;
                let (x, y, r) = (arg(0)?, arg(1)?, arg(2)?);
                let sa = self.int_ws(x).1;
                let sb = self.int_ws(y).1;
                let rq = self.ast().exprs.get(r).ty();
                let pq = self.ast().types.pointee(rq)?;
                let rt = self.ty(pq);
                let (wr, sr) = match self.kind(rt) {
                    TyKind::Int { width, signed } => (u32::from(width), signed),
                    _ => (1, false),
                };
                let a = self.rv(x);
                let b = self.rv(y);
                let p = self.rv(r);
                let sub = u32::from(k)
                    | u32::from(sa) << 2
                    | u32::from(sb) << 3
                    | u32::from(sr) << 4
                    | wr << 6;
                let v = self.reg();
                self.emit_site(
                    Op::Builtin,
                    bi::OVERFLOW | (sub | bi::VALUE) << 8,
                    &[v, a, b, dst, 0, 0],
                    e,
                    begin,
                );
                op(self, bi::OVERFLOW, sub, [a, b, dst], [0, 0], begin);
                self.emit_site(Op::Store, ak::ASSIGN, &[p, v, rt.0], e, begin);
            }
            // WP-557: the carry builtins (clang `IntExprEvaluator`
            // `BI__builtin_addcb`..): the three operands, then the
            // carry-out pointer, are evaluated; the carry-out is stored
            // through the pointer and the wrapped result is the value.
            _ if id::carry_op(bid).is_some() => {
                let minus = id::carry_op(bid)?;
                let (x, y, ci, r) = (arg(0)?, arg(1)?, arg(2)?, arg(3)?);
                let w = self.int_width(x);
                let rq = self.ast().exprs.get(r).ty();
                let pq = self.ast().types.pointee(rq)?;
                let rt = self.ty(pq);
                let a = self.rv(x);
                let b = self.rv(y);
                let c = self.rv(ci);
                let p = self.rv(r);
                let sub = w | u32::from(minus) << 8;
                let co = self.reg();
                self.emit_site(Op::Builtin, bi::CARRY | sub << 8, &[co, a, b, c, 0, 0], e, begin);
                op(self, bi::CARRY, sub | bi::CARRY_VALUE, [a, b, c], [0, 0], begin);
                self.emit_site(Op::Store, ak::ASSIGN, &[p, co, rt.0], e, begin);
            }
            // WP-416: clang folds the alignment builtins on an integer
            // source (`IntExprEvaluator`), checking the alignment (evaluated
            // first) with its note at the alignment argument. WP-432: on a
            // pointer source (`PointerExprEvaluator`; `is_aligned` in
            // `IntExprEvaluator`) from the known alignment of its object,
            // `align_up` / `align_down` evaluating the pointer first; an
            // unknown result is noted at the pointer (spec/oven.md
            // "Builtins").
            id::ALIGN_UP | id::ALIGN_DOWN | id::IS_ALIGNED => {
                let (x, al) = (arg(0)?, arg(1)?);
                let k = match bid {
                    id::ALIGN_UP => 0,
                    id::ALIGN_DOWN => 1,
                    _ => 2,
                };
                let (aw, asg) = self.int_ws(al);
                let at = self.begin(al);
                let t2 = aw | u32::from(asg) << 8;
                let xt = self.ety(x);
                let Some((w, s)) = int_of(self.kind(xt)) else {
                    let q = self.ast().exprs.get(x).ty();
                    let Some(et) = self.ast().types.pointee(q) else {
                        self.unsupported(e);
                        return Some(self.finish_builtin(dst));
                    };
                    let t = self.ty(et).0;
                    // The check reads only the alignment's width, at 64.
                    let ws = 64;
                    let a = if k == 2 {
                        let b = self.rv(al);
                        op(self, bi::ALIGN, ws | 3 << 9, [dst, b, dst], [0, t2], at);
                        let a = self.rv(x);
                        (a, b)
                    } else {
                        let a = self.rv(x);
                        let b = self.rv(al);
                        op(self, bi::ALIGN, ws | 3 << 9, [dst, b, dst], [0, t2], at);
                        (a, b)
                    };
                    let loc = self.ast().exprs.get(x).loc();
                    op(self, bi::PTR_ALIGN, k, [a.0, a.1, dst], [t, 0], loc);
                    return Some(self.finish_builtin(dst));
                };
                let ws = w | u32::from(s) << 8;
                let b = self.rv(al);
                op(self, bi::ALIGN, ws | 3 << 9, [dst, b, dst], [0, t2], at);
                let a = self.rv(x);
                op(self, bi::ALIGN, ws | k << 9, [a, b, dst], [0, t2], at);
            }
            // WP-416: clang 22 folds the element-wise `abs` (integer: the
            // most negative value wraps; floating: `fabs`) and the integer
            // `max` / `min`; its floating `max` / `min` fail silently. clang
            // 19 folds none of them (spec/oven.md "Vectors").
            id::ELEMENTWISE_ABS
            | id::ELEMENTWISE_MAX
            | id::ELEMENTWISE_MIN
            | id::ELEMENTWISE_ADD_SAT
            | id::ELEMENTWISE_SUB_SAT => {
                if self.o.clang_compat < 22 {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                }
                let t = self.ety(e);
                match self.kind(t) {
                    k @ (TyKind::Int { .. } | TyKind::Bool) => {
                        let (w, s) = int_of(k).unwrap_or((1, false));
                        let a = self.rv(arg(0)?);
                        let b = match arg(1) {
                            Some(y) => self.rv(y),
                            None => a,
                        };
                        self.int_elementwise(bid, w, s, [a, b], dst, e);
                    }
                    TyKind::Float(fk) if bid == id::ELEMENTWISE_ABS => {
                        let a = self.rv(arg(0)?);
                        op(self, bi::FP_OP, float_code(fk) | bi::FABS << 4, [a, a, dst], [0, 0], begin);
                    }
                    TyKind::Float(_) => self.trap(e, ""),
                    _ => self.unsupported(e),
                }
            }
            // clang 19 does not constant-evaluate the vector reductions;
            // clang 22 folds the integer ones (spec/oven.md "Vectors").
            id::REDUCE_ADD..=id::REDUCE_XOR if self.o.clang_compat >= 22 => {
                let x = arg(0)?;
                self.vec_reduce(e, bid, x, dst);
            }
            id::REDUCE_ADD..=id::REDUCE_XOR => {
                self.trap_loc(begin, "subexpression not valid in a constant expression".into());
            }
            _ => {
                self.next = dst;
                return None;
            }
        }
        Some(self.finish_builtin(dst))
    }

    /// The object argument of `__atomic_is_lock_free`/`always_lock_free`:
    /// whether it is a null pointer constant (any suitably-sized object
    /// qualifies), else the alignment of its pointee type before the
    /// implicit conversion to `const volatile void *` (0: incomplete).
    fn lock_free_object(&mut self, p: ExprId) -> (bool, u32) {
        let ast = self.ast();
        let mut x = skip_parens(ast, p);
        while ast.exprs.get(x).kind() == ExprKind::ImplicitCastExpr {
            if ast.exprs.cast_kind(x) == kiln_ast::expr::CastKind::NullToPointer {
                return (true, 0);
            }
            x = skip_parens(ast, ast.exprs.operand(x));
        }
        let Some(pointee) = ast.types.pointee(ast.exprs.get(x).ty()) else { return (false, 0) };
        if ast.types.is_void(pointee) {
            return (false, 0);
        }
        // The pointee as written (`_Aligned_Tp *`): an aligned typedef
        // raises its alignment (clang `getTypeAlignInChars`).
        let mut w = ast.exprs.get(x).ty();
        let written = loop {
            if let TypeView::Pointer(p) = ast.types.view(w.ty()) {
                break p;
            }
            let d = ast.types.desugar_once(w);
            if d.ty() == w.ty() {
                break pointee;
            }
            w = d;
        };
        if let Some(a) = self.typedef_align(written) {
            return (false, a);
        }
        let t = self.ty(pointee);
        let info = self.o.types.info(t);
        (false, if info.size == 0 { 0 } else { info.align })
    }

    /// The integer operand of the object argument `p` of
    /// `__atomic_is_lock_free` when `p` is an integer cast to a pointer
    /// (through implicit and pointer-to-pointer casts), clang 19's
    /// `EvaluateAsRValue` + `toIntegralConstant` of the argument.
    fn lock_free_int(&self, p: ExprId) -> Option<ExprId> {
        use kiln_ast::expr::CastKind as K;
        let ast = self.ast();
        let mut x = skip_parens(ast, p);
        for _ in 0..16 {
            if !ast.exprs.get(x).kind().is_cast() {
                return None;
            }
            match ast.exprs.cast_kind(x) {
                K::IntegralToPointer => return Some(ast.exprs.operand(x)),
                K::BitCast | K::NoOp => x = skip_parens(ast, ast.exprs.operand(x)),
                _ => return None,
            }
        }
        None
    }

    /// WP-630: the x86 builtin call `e` (clang 19's
    /// `IntExprEvaluator::VisitBuiltinCallExpr`): `lzcnt` / `tzcnt` (the
    /// width at zero), `bextr` / `bextri`, `bzhi`, `pdep` and `pext`, and
    /// at `-fclang-compat=22` `addcarryx` / `subborrow` (clang 20: the
    /// carry-in counts as 1 when nonzero; the operands, then the pointer,
    /// are evaluated, the wrapped result is stored and the carry-out is
    /// the value). Any other x86 builtin is not a constant expression.
    fn x86_builtin_call(&mut self, e: ExprId, bid: u32, args: &[ExprId], dst: u32) -> u32 {
        let begin = self.begin(e);
        let name = id::info(bid).map_or("", |i| i.name);
        let n = name.strip_prefix(id::x86::PREFIX).unwrap_or(name);
        let op = |s: &mut Self, k: u32, sub: u32, r: [u32; 3]| {
            s.emit_site(Op::Builtin, k | sub << 8, &[dst, r[0], r[1], r[2], 0, 0], e, begin);
        };
        let int_op = match n {
            "bextr_u32" | "bextr_u64" | "bextri_u32" | "bextri_u64" => Some(bi::BEXTR),
            "bzhi_si" | "bzhi_di" => Some(bi::BZHI),
            "pdep_si" | "pdep_di" => Some(bi::PDEP),
            "pext_si" | "pext_di" => Some(bi::PEXT),
            _ => None,
        };
        if let (Some(k), &[x, y]) = (int_op, args) {
            let w = self.int_width(x);
            let a = self.rv(x);
            let b = self.rv(y);
            op(self, bi::INT_OP, w | k << 8, [a, b, dst]);
            return self.finish_builtin(dst);
        }
        match (n, args) {
            (
                "lzcnt_u16" | "lzcnt_u32" | "lzcnt_u64" | "tzcnt_u16" | "tzcnt_u32" | "tzcnt_u64",
                &[x],
            ) => {
                let w = self.int_width(x);
                let a = self.rv(x);
                let fb = self.reg();
                self.konst(fb, u128::from(w));
                let k = if n.starts_with('l') { bi::CLZ } else { bi::CTZ };
                op(self, k, w | 1 << 8, [a, fb, dst]);
            }
            (
                "addcarryx_u32" | "addcarryx_u64" | "subborrow_u32" | "subborrow_u64",
                &[ci, x, y, r],
            ) if self.o.clang_compat >= 22 => {
                let w = self.int_width(x);
                let Some(rt) = self.ast().types.pointee(self.ast().exprs.get(r).ty()) else {
                    self.trap_loc(begin, INVALID.into());
                    return self.finish_builtin(dst);
                };
                let rt = self.ty(rt);
                let c = self.rv(ci);
                let a = self.rv(x);
                let b = self.rv(y);
                let p = self.rv(r);
                let sub = w | u32::from(n.starts_with("sub")) << 8 | bi::CARRY_BOOL_IN;
                let v = self.reg();
                self.emit_site(
                    Op::Builtin,
                    bi::CARRY | (sub | bi::CARRY_VALUE) << 8,
                    &[v, a, b, c, 0, 0],
                    e,
                    begin,
                );
                op(self, bi::CARRY, sub, [a, b, c]);
                self.emit_site(Op::Store, ak::ASSIGN, &[p, v, rt.0], e, begin);
            }
            _ => self.trap_loc(begin, INVALID.into()),
        }
        self.finish_builtin(dst)
    }

    fn finish_builtin(&mut self, dst: u32) -> u32 {
        self.next = dst + 1;
        dst
    }
}

#[cfg(test)]
mod tests {
    use super::parse_auto_radix as p;

    /// clang's `__builtin_nan` payload parsing ([library.c]; clang
    /// `TryEvaluateBuiltinNaN`).
    #[test]
    fn nan_payload_radix() {
        assert_eq!(p(b"0"), Some(0));
        assert_eq!(p(b"12"), Some(12));
        assert_eq!(p(b"0x1F"), Some(31));
        assert_eq!(p(b"0X1f"), Some(31));
        assert_eq!(p(b"0b101"), Some(5));
        assert_eq!(p(b"0o17"), Some(15));
        assert_eq!(p(b"017"), Some(15));
        assert_eq!(p(b"08"), None);
        assert_eq!(p(b"0x"), None);
        assert_eq!(p(b"-1"), None);
        assert_eq!(p(b"1a"), None);
    }
}
