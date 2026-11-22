//! Library fast paths (WP-1864; spec/oven.md "Library fast paths"): calls
//! of standard-library plumbing the Oven evaluates without running the
//! library's code.
//!
//! - A call of a wrapper of the evaluation context (libstdc++'s
//!   `std::__is_constant_evaluated` and `std::is_constant_evaluated`,
//!   `if consteval { return true; } else { return false; }`; libc++'s
//!   `std::__libcpp_is_constant_evaluated`, `return
//!   __builtin_is_constant_evaluated();`) is the context itself, read as
//!   `if consteval` reads it ([`Compiler::ice_wrapper`]): no call, no
//!   frame, no step, no depth, no memo lookup, as clang's `isStdBuiltin`
//!   set is no call.
//! - A call of `std::char_traits<C>::compare`, `length` or `find` for a
//!   character type `C` ([`Compiler::traits_fast`]) is preceded by a
//!   `Builtin` `bi::TRAITS` that evaluates the whole operation natively
//!   (`vm/fast.rs`) and skips the call when it may; when it may not (an
//!   invalid read, a pointer past its array, a budget it would cross), it
//!   declines and the call runs, so every diagnostic is the library
//!   code's.

use kiln_ast::DeclId;
use kiln_ast::expr::{CastKind, ExprKind, StorageDuration};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::{ArgView, BuiltinType};
use kiln_ast::{Ast, ExprId, QualType, StmtId};

use super::new_delete::{in_std, is_named, is_std_named};
use super::{Access, Compiler, Place, regs, skip_parens};
use crate::kbc::Op;
use crate::kbc::bi;
use crate::mem::Origin;
use crate::ty::OTy;

/// The statement a compound statement of one statement holds (or `s`).
fn only(ast: &Ast, s: StmtId) -> Option<StmtId> {
    if ast.stmts.get(s).kind() != StmtKind::CompoundStmt {
        return Some(s);
    }
    match ast.lists.get(ast.stmts.compound_body(s))[..] {
        [x] => Some(StmtId::new(x)),
        _ => None,
    }
}

/// `e` without parentheses and no-op or boolean-preserving implicit
/// conversions.
fn bare(ast: &Ast, mut e: ExprId) -> ExprId {
    loop {
        e = skip_parens(ast, e);
        let n = ast.exprs.get(e);
        if n.kind() == ExprKind::ImplicitCastExpr
            && matches!(ast.exprs.cast_kind(e), CastKind::NoOp | CastKind::LValueToRValue)
        {
            e = ast.exprs.operand(e);
            continue;
        }
        return e;
    }
}

/// The `bool` literal statement `s` returns (`return true;`, alone or in
/// braces).
fn returns_bool(ast: &Ast, s: StmtId) -> Option<bool> {
    let r = only(ast, s)?;
    if ast.stmts.get(r).kind() != StmtKind::ReturnStmt {
        return None;
    }
    let v = bare(ast, ast.stmts.return_value(r)?);
    (ast.exprs.get(v).kind() == ExprKind::CXXBoolLiteralExpr).then(|| ast.exprs.bool_value(v))
}

impl Compiler<'_> {
    /// Whether `d` is a library wrapper of the evaluation context
    /// (spec/oven.md "Library fast paths"): a function of namespace `std`
    /// without parameters returning `bool`, whose definition is one
    /// `if consteval` (or `if !consteval`) returning a different `bool`
    /// literal from each branch, or one `return
    /// __builtin_is_constant_evaluated();` (not in `std::is_constant_evaluated`
    /// itself, whose builtin call warns, `-Wconstant-evaluated`). `Some(neg)`:
    /// the call's value is the context, negated when `neg`.
    pub(super) fn ice_wrapper(&self, d: DeclId) -> Option<bool> {
        let ast = self.ast();
        if !in_std(ast, d) {
            return None;
        }
        let def = ast.definition(d)?;
        let f = ast.decls.function(def)?;
        if !ast.lists.get(f.params).is_empty() {
            return None;
        }
        let ret = match ast.types.canonical_view(ast.decls.get(def).ty()?) {
            kiln_ast::types::TypeView::Function(p) => p.ret(),
            _ => return None,
        };
        if ast.types.builtin(ret) != Some(BuiltinType::Bool) {
            return None;
        }
        let s = only(ast, f.body?)?;
        match ast.stmts.get(s).kind() {
            StmtKind::IfStmt => {
                let kind = ast.stmts.if_kind(s);
                let x = ast.stmts.if_data(s);
                if !matches!(kind, IfKind::Consteval | IfKind::NotConsteval)
                    || x.init.is_some()
                    || x.cond_var.is_some()
                {
                    return None;
                }
                let (t, e) = (returns_bool(ast, x.then?)?, returns_bool(ast, x.els?)?);
                // `if consteval`: `t` in a constant context, `e` outside.
                (t != e).then_some(t == (kind == IfKind::NotConsteval))
            }
            StmtKind::ReturnStmt => {
                let v = bare(ast, ast.stmts.return_value(s)?);
                if is_named(ast, d, "is_constant_evaluated")
                    || ast.exprs.get(v).kind() != ExprKind::CallExpr
                    || !ast.lists.get(ast.exprs.call_args(v)).is_empty()
                {
                    return None;
                }
                let mut c = skip_parens(ast, ast.exprs.callee(v));
                while ast.exprs.get(c).kind().is_cast() {
                    c = skip_parens(ast, ast.exprs.operand(c));
                }
                (ast.exprs.get(c).kind() == ExprKind::DeclRefExpr
                    && ast.decls.builtin_id(ast.exprs.decl(c))
                        == Some(kiln_ast::decl::builtin::IS_CONSTANT_EVALUATED))
                .then_some(false)
            }
            _ => None,
        }
    }

    /// The `bi::TRAITS` sub and element type of a call of `d` with
    /// `nargs` arguments and no object argument, when `d` is
    /// `std::char_traits<C>::compare(const C*, const C*, size_t)`,
    /// `length(const C*)`, `find(const C*, size_t, const C&)` or
    /// `assign(C&, const C&)` (WP-1918) for a character type `C` (`char`,
    /// `wchar_t`, `char8_t`, `char16_t`, `char32_t`): the standard fixes
    /// what each computes ([char.traits.require]), so the library's
    /// definition need not be read.
    pub(super) fn traits_fast(&mut self, d: DeclId, nargs: usize) -> Option<(u32, OTy)> {
        let ast = self.ast();
        let rec = ast.decls.get(d).ctx()?;
        if !is_std_named(ast, rec, "char_traits") {
            return None;
        }
        let kind = [
            ("compare", bi::TR_COMPARE, 3),
            ("length", bi::TR_LENGTH, 1),
            ("find", bi::TR_FIND, 3),
            ("assign", bi::TR_ASSIGN, 2),
        ]
        .into_iter()
        .find(|&(n, _, k)| k == nargs && is_named(ast, d, n))
        .map(|(_, k, _)| k)?;
        let f = ast.decls.function(ast.first(d))?;
        let params = ast.lists.get(f.params);
        if params.len() != nargs {
            return None;
        }
        let p0 = ast.decls.get(DeclId::new(params[0])).ty()?;
        // `assign` takes its target by reference, the others by pointer.
        let by_ref = kind == bi::TR_ASSIGN;
        let elem = ast.types.pointee(p0).filter(|_| ast.types.is_reference(p0) == by_ref)?;
        if by_ref && !ast.types.effective_quals(elem).is_empty() {
            return None;
        }
        let unsigned = match ast.types.builtin(elem)? {
            BuiltinType::Char | BuiltinType::Char8 => bi::TR_UNSIGNED,
            BuiltinType::WChar | BuiltinType::Char16 | BuiltinType::Char32 => 0,
            _ => return None,
        };
        let et = self.ty(elem.without_quals());
        Some((bi::TRAITS | (kind | unsigned) << 8, et))
    }

    /// The `bi::MINMAX` sub and operand type of a call of `d` with
    /// `nargs` arguments returning a glvalue ([`minmax_call`]).
    pub(super) fn minmax_fast(&mut self, d: DeclId, nargs: usize) -> Option<(u32, OTy)> {
        let (sub, t) = minmax_call(self.ast(), d, nargs)?;
        Some((bi::MINMAX | sub << 8, self.ty(t.without_quals())))
    }

    /// `call`, an lvalue-to-rvalue conversion's operand that
    /// [`minmax_args`] took, with arguments `args`, evaluated for its
    /// value: the `bi::MINMAX` builtin with `MM_VALUE` reads a register
    /// local's register and a scalar temporary's value where they are
    /// (the scan left the local in a register, spec/oven.md "Register
    /// locals"), and the address of any other argument. Returns the place
    /// of the declined call's result, the `JmpIf` taken when the builtin
    /// evaluated the call, and the register holding its value; `None`
    /// (nothing emitted) when no argument is such a value.
    pub(super) fn minmax_value(
        &mut self,
        call: ExprId,
        args: [ExprId; 2],
    ) -> Option<(Place, usize, u32)> {
        let ast = self.ast();
        // [`minmax_args`] looks through parentheses, `(std::min(a, b))`
        // (WP-2061): the call is the expression inside them.
        let call = skip_parens(ast, call);
        let d = regs::direct_callee(ast, call)?;
        let value = |c: &Self, a: ExprId| {
            let x = bare_lvalue(ast, a);
            match ast.exprs.get(x).kind() {
                ExprKind::DeclRefExpr => {
                    matches!(c.locals.get(&ast.exprs.decl(x)), Some(super::Local::Reg(_)))
                }
                ExprKind::MaterializeTemporaryExpr => scalar_temp(c, x),
                _ => false,
            }
        };
        if !value(self, args[0]) && !value(self, args[1]) {
            return None;
        }
        let (aux, ot) = self.minmax_fast(d, 2)?;
        // Each argument as the call evaluates it, in order: a value
        // (`MM_VAL_A`/`MM_VAL_B`), or an address.
        let mut ops = [(0, false); 2];
        let mut pre = Vec::new();
        for (i, &a) in args.iter().enumerate() {
            let x = bare_lvalue(self.ast(), a);
            ops[i] = match self.node(x).kind() {
                ExprKind::DeclRefExpr => match self.lv(x, Access::Addr) {
                    Place::Reg(r) => (r, true),
                    Place::Mem(p) => (p, false),
                    _ => (self.lv_mem(a, Access::Addr), false),
                },
                ExprKind::MaterializeTemporaryExpr if scalar_temp(self, x) => {
                    let inner = self.ast().exprs.operand(x);
                    let r = self.rv(inner);
                    // The declined call binds its parameter to the
                    // temporary holding this value, as `mte` makes it.
                    let t = self.reg();
                    let is_const = super::const_object(self.ast(), self.ast().exprs.get(x).ty());
                    pre.push((a, t, r, inner, is_const));
                    (r, true)
                }
                _ => (self.lv_mem(a, Access::Addr), false),
            };
        }
        let base = self.next;
        self.reg();
        self.reg();
        self.emit(Op::Mov, 0, &[base, ops[0].0]);
        self.emit(Op::Mov, 0, &[base + 1, ops[1].0]);
        let flags = bi::MM_VALUE
            | if ops[0].1 { bi::MM_VAL_A } else { 0 }
            | if ops[1].1 { bi::MM_VAL_B } else { 0 };
        let ok = self.reg();
        self.emit(Op::Builtin, aux | flags << 8, &[ok, base, 0, 0, ot.0, 0]);
        let done = self.jmp_if(ok, true);
        // Declined: the call, its reference parameters bound to the
        // arguments evaluated above (`bind_ref`; a register local is
        // copied to a temporary there).
        let mut bound = Vec::new();
        for &(a, t, r, inner, is_const) in &pre {
            self.alloc(t, ot, Origin::Temp(inner), is_const, inner);
            self.emit(Op::InitStore, 0, &[t, r, ot.0]);
            bound.push((a, t));
        }
        for (i, &a) in args.iter().enumerate() {
            if !ops[i].1 {
                bound.push((a, ops[i].0));
            }
        }
        let saved = std::mem::replace(&mut self.bound_args, bound);
        let p = self.lv(call, Access::Read);
        self.bound_args = saved;
        Some((p, done, base))
    }
}

/// `e` through parentheses and the no-op glvalue casts that add `const`
/// (an lvalue argument bound to a `const T&` parameter).
pub(super) fn bare_lvalue(ast: &Ast, e: ExprId) -> ExprId {
    let mut e = skip_parens(ast, e);
    loop {
        let n = ast.exprs.get(e);
        if !(n.kind().is_cast()
            && ast.exprs.cast_kind(e) == CastKind::NoOp
            && n.value_category().is_glvalue())
        {
            return e;
        }
        e = skip_parens(ast, ast.exprs.operand(e));
    }
}

/// Whether `x`, a `MaterializeTemporaryExpr`, is a scalar temporary of
/// its full-expression, not lifetime-extended.
fn scalar_temp(c: &Compiler<'_>, x: ExprId) -> bool {
    let ast = c.ast();
    ast.exprs.temporary_storage(x) == StorageDuration::FullExpression
        && !c.ext_temps.contains_key(&x)
        && ast.types.is_arithmetic(ast.exprs.get(x).ty())
}

/// The arguments of `e` when it is a call [`minmax_call`] takes with two
/// arguments (the operand of an lvalue-to-rvalue conversion: the
/// register-locals scan reads a local named by one where it is, and
/// `Compiler::minmax_value` evaluates the call for its value).
pub(super) fn minmax_args(ast: &Ast, e: ExprId) -> Option<[ExprId; 2]> {
    let e = skip_parens(ast, e);
    let n = ast.exprs.get(e);
    if n.kind() != ExprKind::CallExpr || !n.value_category().is_glvalue() {
        return None;
    }
    let [a, b] = ast.lists.get(ast.exprs.call_args(e))[..] else { return None };
    minmax_call(ast, regs::direct_callee(ast, e)?, 2)?;
    Some([ExprId::new(a), ExprId::new(b)])
}

/// The `bi::MINMAX` sub and operand type `T` of a call of `d` with
/// `nargs` arguments, when `d` is `std::min` or `std::max` of two
/// `const T&` (WP-1917), `T` an arithmetic builtin type (no enumeration,
/// no pointer, no `_BitInt`), with no comparator or a `std::less<U>` or
/// `std::greater<U>` one, `U` `T` or `void`: the standard fixes the
/// result ([alg.min.max]: the first argument when neither is less than
/// the other) and these comparators call no program code.
pub(super) fn minmax_call(ast: &Ast, d: DeclId, nargs: usize) -> Option<(u32, QualType)> {
    let mut sub = match () {
        () if is_std_named(ast, d, "min") => 0,
        () if is_std_named(ast, d, "max") => bi::MM_MAX,
        () => return None,
    };
    if nargs != 2 && nargs != 3 {
        return None;
    }
    let f = ast.decls.function(ast.first(d))?;
    let params = ast.lists.get(f.params);
    if params.len() != nargs {
        return None;
    }
    let pty = |i: usize| ast.decls.get(DeclId::new(params[i])).ty();
    let referent = |q: QualType| ast.types.pointee(q).filter(|_| ast.types.is_reference(q));
    let (t, u) = (referent(pty(0)?)?, referent(pty(1)?)?);
    let quals = ast.types.effective_quals(t);
    if !quals.is_const()
        || quals.is_volatile()
        || ast.types.builtin(t).is_none()
        || !ast.types.is_arithmetic(t)
        || !ast.types.same_unqualified(t, u)
    {
        return None;
    }
    if nargs == 3 {
        let c = pty(2)?;
        let rec = ast.types.as_record(c).filter(|_| !ast.types.is_reference(c))?;
        if is_std_named(ast, rec, "greater") {
            sub |= bi::MM_GREATER;
        } else if !is_std_named(ast, rec, "less") {
            return None;
        }
        let (_, info) = ast.spec_info(rec)?;
        let ArgView::Type(x) = ast.types.args(info.args).first()?.view() else {
            return None;
        };
        if !ast.types.is_void(x) && !ast.types.same_unqualified(x, t) {
            return None;
        }
    }
    Some((sub, t))
}
