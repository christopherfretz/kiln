//! "constexpr function never produces a constant expression" at a
//! definition ([dcl.constexpr]/6 before C++23, no diagnostic required;
//! clang 19 `CheckConstexprFunctionBody` -> `Expr::isPotentialConstantExpr`
//! under `-Winvalid-constexpr`: an error by default before C++23, an
//! off-by-default warning from C++23; spec/sema.md "Never constant").
//!
//! clang evaluates the body once with the parameters (and `this`)
//! unknown, going on after a failure to read one, and reports the first
//! failure that does not stem from an unknown value, in the function's
//! own body. A call of a constexpr function is not evaluated at all
//! (clang `CheckCallLimit` refuses any nested call), so its value is
//! unknown and a failure inside a callee is never reported; the oven's
//! potential mode (`Oven::evaluate_potential`, and `evaluate_call` past
//! its own frame) makes such a call fail quietly. kiln follows clang in
//! two ways:
//! - a non-member or static member function without parameters that
//!   returns a scalar or `void` is evaluated as a call
//!   ([`Sema::evaluate_call`]): every failure in its own body counts;
//! - any other function's body is walked in execution order as far as
//!   the path is known: a statement or initializer that names no
//!   parameter, local variable or `this` is evaluated on its own
//!   ([`Sema::eval_potential`]); one that does is taken apart operand by
//!   operand, as `enable_if` conditions are (`attr_cond.rs`). A loop
//!   whose condition is absent or known true has its first iteration
//!   walked; the walk stops at an unknown condition, a `switch`, a `try`,
//!   a label or a `return`; a constructor's body is walked only if each of its
//!   mem-initializers (default member initializers included) is known.
//!
//! The walk treats every local variable as unknown, where clang tracks
//! their values, so it misses some functions clang reports; it never
//! reports one clang accepts (WP-873).

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind};
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ExprId, StmtId};
use kiln_diag::DiagId;
use kiln_oven::{EvalFailure, FailKind, Note};
use kiln_source::Loc;

use crate::Sema;

/// The deepest statement or expression nesting the walk takes apart.
const MAX_DEPTH: u32 = 64;

/// An expression's outcome with the parameters and locals unknown.
enum Value {
    /// A constant.
    Known(ConstValueId),
    /// It may be a constant for some parameter values.
    Unknown,
    /// It is never a constant; the notes say why.
    Fails(EvalFailure),
}

/// What [`Sema::clang_constexpr_builtin`] is asked besides the builtin's
/// name: the target, dialect and `-fclang-compat` level its answer
/// depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinQuery {
    /// An `x86_64` target (else `AArch64`).
    pub x86_64: bool,
    /// The language standard.
    pub std: kiln_lex::Std,
    /// A GNU dialect (`-std=gnu++NN`, clang `GNUMode`).
    pub gnu: bool,
    /// The `-fclang-compat` level (19 or 22).
    pub clang_compat: u32,
}

/// How clang 19's evaluator treats a direct call of a function, by the
/// `Constexpr` markers of `Builtins.td` (`__has_constexpr_builtin`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BuiltinCall {
    /// Not a builtin: an ordinary call.
    None,
    /// A builtin clang constant-evaluates, or one clang does not know:
    /// its own evaluation decides, which kiln's Oven need not mirror, so
    /// the call may be constant (clang `VisitBuiltinCallExpr`).
    Opaque,
    /// A builtin clang does not constant-evaluate (`__builtin_floor`,
    /// `__builtin_trap`): evaluating the callee, before the arguments,
    /// fails ("subexpression not valid in a constant expression").
    Invalid,
    /// A library builtin clang constant-evaluates (`strlen`, `memcpy`):
    /// clang notes its library spelling as a non-constexpr function
    /// before evaluating it, whatever the arguments.
    LibEvaluated(&'static str),
    /// Any other library builtin (`fabs`, `abs`): a call of an ordinary
    /// non-constexpr function, which fails once its arguments are known
    /// (kiln's Oven folds some of them, as GCC does).
    LibPlain,
}

/// The kinds of builtin an expression calls ([`Sema::builtin_calls`]).
#[derive(Clone, Copy, Default)]
struct BuiltinCalls {
    /// An [opaque](BuiltinCall::Opaque) one.
    opaque: bool,
    /// One whose failure kiln's Oven may not show: an
    /// [invalid](BuiltinCall::Invalid) one or a library builtin.
    strict: bool,
}

/// A statement's outcome.
enum Flow {
    /// Execution goes on with the next statement.
    Next,
    /// The rest of the path is not known (or the function returned).
    Stop,
    /// It is never a constant.
    Fails(EvalFailure),
}

impl Sema<'_, '_> {
    /// The never-constant check of `f`, a function (or the call operator
    /// of a lambda declared `constexpr`/`consteval` explicitly) whose
    /// definition with `body` was just finished (see the module docs).
    pub(crate) fn check_never_constant(&mut self, f: DeclId, body: StmtId) {
        let kind = self.fn_spec(f).constexpr_kind();
        if kind == ConstexprKind::None
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || !self.inst.stack.frames.is_empty()
            || self.in_template()
            || !self
                .diags
                .group_flag("invalid-constexpr")
                .unwrap_or(self.std < kiln_lex::Std::Cxx23)
        {
            return;
        }
        let dk = self.kind_of(f);
        if !matches!(
            dk,
            DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        ) {
            return;
        }
        let loc = self.ast.decls.get(f).loc();
        let fail = if self.called_without_arguments(f) {
            // kiln's Oven folds some builtins clang does not evaluate
            // (`__builtin_floor`, `fabs`) and notes others its own way
            // (`strlen`): a call of such a builtin on the known path
            // decides first.
            self.nc_strict_only = true;
            let strict = self.never_constant_body(f, body);
            self.nc_strict_only = false;
            strict.or_else(|| self.never_constant_call(f, loc))
        } else {
            self.never_constant_body(f, body)
        };
        let Some(fail) = fail else { return };
        let msg = self
            .diag(DiagId::sema_constexpr_never_constant, loc)
            .arg(u8::from(dk == DeclKind::CXXConstructor))
            .arg(u8::from(kind == ConstexprKind::Consteval));
        let msg = Self::with_failure_notes(msg, fail);
        self.emit(msg);
    }

    /// Whether `f` is a non-member or static member function without
    /// parameters returning a scalar or `void` (what
    /// [`Sema::evaluate_call`] evaluates).
    fn called_without_arguments(&self, f: DeclId) -> bool {
        if self.kind_of(f) == DeclKind::CXXMethod && !self.is_static_method(f) {
            return false;
        }
        if !matches!(self.kind_of(f), DeclKind::Function | DeclKind::CXXMethod) {
            return false;
        }
        let Some(fty) = self.ast.decls.get(f).ty() else { return false };
        let types = &self.ast.types;
        let TypeView::Function(p) = types.canonical_view(fty) else { return false };
        let ret = p.ret();
        p.num_params() == 0
            && !p.variadic()
            && (types.is_void(ret)
                || types.is_arithmetic(ret)
                || types.is_pointer(ret)
                || matches!(types.canonical_view(ret), TypeView::Enum(_)))
    }

    /// The failure of a call of `f` without arguments, in its own body.
    fn never_constant_call(&mut self, f: DeclId, loc: Loc) -> Option<EvalFailure> {
        let mut fail = self.evaluate_call(f, loc).err()?;
        // The call's own "in call to" note (a "declared here" note may
        // follow it).
        let own = |n: &Note| n.loc == loc && n.msg.starts_with("in call to ");
        let i = fail.notes.iter().rposition(own)?;
        fail.notes.remove(i);
        Self::reportable(&fail).then_some(fail)
    }

    /// Whether failure `fail` of an evaluation in the function's own body
    /// is one clang reports: not a failure in a callee ("in call to"),
    /// nor one clang's potential-constant evaluation does not diagnose
    /// (a read of an uninitialized object, a variable without a known
    /// initializer, an undefined function, a leak, an exception a
    /// calling constant expression may catch).
    fn reportable(fail: &EvalFailure) -> bool {
        !fail.notes.is_empty()
            && !fail.notes.iter().any(|n| {
                n.msg.starts_with("in call to ")
                    || n.msg.contains("uninitialized")
                    || n.msg.contains("was not deallocated")
                    || n.msg.starts_with("initializer of ")
                    || n.msg.starts_with("undefined ")
                    || n.msg.starts_with("unhandled exception")
                    // A kiln limitation, not a clang failure (WP-962).
                    || n.msg.starts_with("kiln:")
            })
    }

    /// The first failure on the known path through `f`'s body (and its
    /// mem-initializers).
    fn never_constant_body(&mut self, f: DeclId, body: StmtId) -> Option<EvalFailure> {
        if self.kind_of(f) == DeclKind::CXXConstructor {
            let inits: Vec<ExprId> = self.ast.ctor_inits(f).map(|ci| ci.init).collect();
            let mut known = true;
            for init in inits {
                match self.nc_expr(init, 0) {
                    Value::Fails(fail) => return Some(fail),
                    Value::Unknown => known = false,
                    Value::Known(_) => {}
                }
            }
            if !known {
                return None;
            }
        }
        match self.nc_stmt(body, 0) {
            Flow::Fails(fail) => Some(fail),
            _ => None,
        }
    }

    /// Whether `e` reads a parameter, a local variable or `this`.
    fn names_unknown(&self, e: ExprId, depth: u32) -> bool {
        if depth > 256 {
            return true;
        }
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::CXXThisExpr => true,
            ExprKind::DeclRefExpr => self.has_local_storage(ex.decl(e)),
            ExprKind::CXXDefaultInitExpr => {
                let field = ex.default_init_field(e);
                self.ast
                    .decls
                    .field(field)
                    .and_then(|fd| fd.init)
                    .is_some_and(|i| self.names_unknown(i, depth + 1))
            }
            _ => ex
                .children(e, &self.ast.lists)
                .into_iter()
                .any(|c| self.names_unknown(c, depth + 1)),
        }
    }

    /// Whether `e` is a prvalue the walk evaluates on its own: of
    /// arithmetic, enumeration, pointer, `void` or class type.
    fn nc_evaluable(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        let ty = n.ty();
        let types = &self.ast.types;
        !n.value_category().is_glvalue()
            && (types.is_arithmetic(ty)
                || types.is_void(ty)
                || types.is_pointer(ty)
                || types.as_record(ty).is_some()
                || matches!(types.canonical_view(ty), TypeView::Enum(_)))
    }

    /// `e` evaluated on its own, a failure clang would not report being
    /// unknown.
    fn nc_eval(&mut self, e: ExprId) -> Value {
        let at = self.begin_loc(e);
        match self.eval_potential(e, at) {
            Ok(v) => Value::Known(v),
            Err(f) if Self::reportable(&f) => Value::Fails(f),
            Err(_) => Value::Unknown,
        }
    }

    /// A `throw`: clang `Error(E)` (the operand is not evaluated) before
    /// C++26; from C++26 (P3068) a handler in a calling constant
    /// expression may catch it, so it is unknown.
    fn nc_throw(&self, e: ExprId) -> Value {
        if self.std >= kiln_lex::Std::Cxx26 {
            return Value::Unknown;
        }
        Value::Fails(Self::not_valid(self.begin_loc(e)))
    }

    /// `c ? t : f`: a known condition picks the arm; an unknown one fails
    /// only when both arms do (clang `CheckPotentialConstantConditional`).
    fn nc_conditional(&mut self, e: ExprId, depth: u32) -> Value {
        let ex = &self.ast.exprs;
        let c = ex.children(e, &self.ast.lists)[0];
        let [t, f] = ex.cond_arms(e);
        match self.nc_expr(c, depth + 1) {
            Value::Known(v) => match self.const_truth(v) {
                Some(b) => self.nc_expr(if b { t } else { f }, depth + 1),
                None => Value::Unknown,
            },
            Value::Unknown => {
                // clang `CheckPotentialConstantConditional`.
                for arm in [f, t] {
                    if !matches!(self.nc_expr(arm, depth + 1), Value::Fails(_)) {
                        return Value::Unknown;
                    }
                }
                Value::Fails(EvalFailure {
                    kind: FailKind::NotConstant,
                    notes: vec![Note::new(
                        self.begin_loc(e),
                        "both arms of conditional operator are unable to produce a \
                         constant expression",
                    )],
                })
            }
            p @ Value::Fails(_) => p,
        }
    }

    /// `e` evaluated with the parameters, locals and `this` unknown
    /// (clang's evaluator in `checkingPotentialConstantExpression` mode:
    /// every operand of an operator or call is evaluated, except past an
    /// unknown `&&`/`||` left operand or `?:` condition).
    fn nc_expr(&mut self, e: ExprId, depth: u32) -> Value {
        if depth > MAX_DEPTH {
            return Value::Unknown;
        }
        let ex = &self.ast.exprs;
        let kind = ex.get(e).kind();
        if kind == ExprKind::CXXThrowExpr {
            return self.nc_throw(e);
        }
        let evaluable = self.nc_evaluable(e);
        // A call of a builtin clang does not evaluate the way kiln's Oven
        // does (WP-945): the operands decide, never the Oven's value.
        let calls = self.builtin_calls(e, 0);
        if self.nc_strict_only && !calls.strict {
            return Value::Unknown;
        }
        if calls.strict {
            // Taken apart below.
        } else if !self.names_unknown(e, 0) {
            if !evaluable {
                return Value::Unknown;
            }
            match self.nc_eval(e) {
                // A failure that may stem from a builtin clang evaluates
                // but kiln's Oven may not: the operands decide (WP-962).
                Value::Fails(_) if calls.opaque => {}
                v => return v,
            }
        } else if evaluable && let Value::Known(v) = self.nc_eval_trial(e) {
            // An unknown name in an unevaluated operand, or one whose
            // value is not needed (a `const` local usable in constant
            // expressions).
            return Value::Known(v);
        }
        let ex = &self.ast.exprs;
        match kind {
            ExprKind::BinaryOperator
                if matches!(ex.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let or = ex.binary_op(e) == BinaryOp::LOr;
                let [l, r] = ex.operands(e);
                match self.nc_expr(l, depth + 1) {
                    Value::Known(v) => match self.const_truth(v) {
                        Some(t) if t == or => Value::Known(v),
                        Some(_) => self.nc_expr(r, depth + 1),
                        None => Value::Unknown,
                    },
                    p => p,
                }
            }
            ExprKind::ConditionalOperator => self.nc_conditional(e, depth),
            ExprKind::ParenExpr
            | ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::UnaryOperator
            | ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator
            | ExprKind::ArraySubscriptExpr
            | ExprKind::CallExpr
            | ExprKind::CXXMemberCallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::CXXConstructExpr
            | ExprKind::CXXTemporaryObjectExpr
            | ExprKind::CXXParenListInitExpr
            | ExprKind::InitListExpr
            | ExprKind::MemberExpr
            | ExprKind::ConstantExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXDefaultInitExpr => {
                let mut ops = if kind == ExprKind::CXXDefaultInitExpr {
                    let field = ex.default_init_field(e);
                    self.ast.decls.field(field).and_then(|fd| fd.init).into_iter().collect()
                } else {
                    ex.children(e, &self.ast.lists)
                };
                if let Some((callee, f)) = self.nc_direct_callee(e) {
                    // The callee of a direct call is not a value
                    // (clang evaluates it as the call's target, so a
                    // builtin or a consteval function named there is
                    // no failure); the arguments are evaluated.
                    ops.retain(|&c| c != callee);
                    match self.builtin_call_kind(f) {
                        BuiltinCall::None => {}
                        BuiltinCall::Invalid => {
                            return Value::Fails(Self::not_valid(self.begin_loc(callee)));
                        }
                        BuiltinCall::LibEvaluated(name) => {
                            return Value::Fails(EvalFailure {
                                kind: FailKind::NotConstant,
                                notes: vec![Self::non_constexpr_note(self.begin_loc(e), name)],
                            });
                        }
                        BuiltinCall::LibPlain => return self.nc_lib_call(e, f, ops, depth),
                        BuiltinCall::Opaque => {
                            for c in self.opaque_builtin_operands(f, ops) {
                                if let p @ Value::Fails(_) = self.nc_expr(c, depth + 1) {
                                    return p;
                                }
                            }
                            return Value::Unknown;
                        }
                    }
                }
                // Every operand, the first never-constant one deciding.
                for c in ops {
                    if let p @ Value::Fails(_) = self.nc_expr(c, depth + 1) {
                        return p;
                    }
                }
                Value::Unknown
            }
            _ => Value::Unknown,
        }
    }

    /// The callee operand of call `e` and the function it names, when
    /// `e` is a `CallExpr` or `CXXOperatorCallExpr` whose callee is a
    /// (decayed, parenthesized) reference to a function.
    fn nc_direct_callee(&self, e: ExprId) -> Option<(ExprId, DeclId)> {
        let ex = &self.ast.exprs;
        if !matches!(ex.get(e).kind(), ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr) {
            return None;
        }
        let callee = *ex.children(e, &self.ast.lists).first()?;
        let mut r = callee;
        for _ in 0..16 {
            match ex.get(r).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => {
                    r = *ex.children(r, &self.ast.lists).first()?;
                }
                ExprKind::DeclRefExpr => {
                    let d = ex.decl(r);
                    return matches!(
                        self.kind_of(d),
                        DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion
                    )
                    .then_some((callee, d));
                }
                _ => return None,
            }
        }
        None
    }

    /// clang's "subexpression not valid in a constant expression" at
    /// `loc`.
    fn not_valid(loc: Loc) -> EvalFailure {
        EvalFailure {
            kind: FailKind::NotConstant,
            notes: vec![Note::new(loc, "subexpression not valid in a constant expression")],
        }
    }

    /// clang's `note_constexpr_invalid_function` for a call at `loc` of
    /// the non-constexpr function `name`.
    fn non_constexpr_note(loc: Loc, name: &str) -> Note {
        Note::new(
            loc,
            format!("non-constexpr function '{name}' cannot be used in a constant expression"),
        )
    }

    /// The [`BuiltinQuery`] of this translation unit.
    fn builtin_query(&self) -> BuiltinQuery {
        BuiltinQuery {
            x86_64: self.is_x86_target(),
            std: self.std,
            gnu: self.gnu,
            clang_compat: self.clang_compat,
        }
    }

    /// How clang evaluates a direct call of `f` (see [`BuiltinCall`]): a
    /// builtin function's id, or a library builtin's spelling, looked up
    /// in clang's constexpr-builtin table for this target, dialect and
    /// compat level ([`Sema::clang_constexpr_builtin`]).
    fn builtin_call_kind(&mut self, f: DeclId) -> BuiltinCall {
        let q = self.builtin_query();
        if let Some(id) = self.ast.decls.builtin_id(f) {
            let Some(info) = kiln_ast::decl::builtin::info(id) else {
                return BuiltinCall::Opaque;
            };
            return match (self.clang_constexpr_builtin)(info.name, q) {
                Some(false) => BuiltinCall::Invalid,
                _ => BuiltinCall::Opaque,
            };
        }
        if self.kind_of(f) != DeclKind::Function
            || self.fn_spec(f).constexpr_kind() != ConstexprKind::None
        {
            return BuiltinCall::None;
        }
        let Some(info) = self.lib_builtin_info(f) else { return BuiltinCall::None };
        match (self.clang_constexpr_builtin)(info.name, q) {
            Some(true) => BuiltinCall::LibEvaluated(info.name),
            _ => BuiltinCall::LibPlain,
        }
    }

    /// The operands `ops` of a call of the [opaque](BuiltinCall::Opaque)
    /// builtin `f` that clang's evaluator evaluates as such: none for
    /// the builtins whose operand is unevaluated or evaluated only
    /// speculatively (`__builtin_constant_p`, `__builtin_object_size`,
    /// `__builtin_classify_type`, `__builtin_assume`), the first for
    /// `__builtin_expect`, else all.
    fn opaque_builtin_operands(&self, f: DeclId, mut ops: Vec<ExprId>) -> Vec<ExprId> {
        let name = self
            .ast
            .decls
            .builtin_id(f)
            .and_then(kiln_ast::decl::builtin::info)
            .map_or("", |i| i.name);
        match name {
            "__builtin_constant_p"
            | "__builtin_object_size"
            | "__builtin_dynamic_object_size"
            | "__builtin_classify_type"
            | "__builtin_assume"
            | "__assume" => Vec::new(),
            "__builtin_expect" | "__builtin_expect_with_probability" => {
                ops.truncate(1);
                ops
            }
            _ => ops,
        }
    }

    /// Call `e` of [library builtin](BuiltinCall::LibPlain) `f` with
    /// arguments `args`: a call of a non-constexpr function, which fails
    /// once the arguments are known (clang evaluates them first).
    fn nc_lib_call(&mut self, e: ExprId, f: DeclId, args: Vec<ExprId>, depth: u32) -> Value {
        let mut known = true;
        for c in args {
            match self.nc_expr(c, depth + 1) {
                p @ Value::Fails(_) => return p,
                Value::Unknown => known = false,
                Value::Known(_) => {}
            }
        }
        if !known {
            return Value::Unknown;
        }
        let name = self
            .ast
            .decls
            .get(f)
            .name()
            .as_ident()
            .map_or_else(String::new, |n| self.ast.symbols().as_str(n).to_owned());
        let decl = self.ast.decls.get(self.ast.latest(f)).loc();
        Value::Fails(EvalFailure {
            kind: FailKind::NotConstant,
            notes: vec![
                Self::non_constexpr_note(self.begin_loc(e), &name),
                Note::new(decl, "declared here"),
            ],
        })
    }

    /// Which kinds of builtin `e` calls directly, outside unevaluated
    /// operands, lambda bodies and the operands clang does not evaluate
    /// ([`Sema::opaque_builtin_operands`]).
    fn builtin_calls(&mut self, e: ExprId, depth: u32) -> BuiltinCalls {
        let mut out = BuiltinCalls::default();
        if depth > 256 {
            out.opaque = true;
            return out;
        }
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::CXXTypeidExpr
            | ExprKind::LambdaExpr
            | ExprKind::RequiresExpr => return out,
            _ => {}
        }
        let mut ops = ex.children(e, &self.ast.lists);
        if let Some((callee, f)) = self.nc_direct_callee(e) {
            ops.retain(|&c| c != callee);
            match self.builtin_call_kind(f) {
                BuiltinCall::None => {}
                BuiltinCall::Opaque => {
                    out.opaque = true;
                    ops = self.opaque_builtin_operands(f, ops);
                }
                BuiltinCall::Invalid | BuiltinCall::LibEvaluated(_) | BuiltinCall::LibPlain => {
                    out.strict = true;
                }
            }
        }
        for c in ops {
            let sub = self.builtin_calls(c, depth + 1);
            out.opaque |= sub.opaque;
            out.strict |= sub.strict;
        }
        out
    }

    /// `e`, which names an unknown, evaluated on its own: only a success
    /// counts.
    fn nc_eval_trial(&mut self, e: ExprId) -> Value {
        let at = self.begin_loc(e);
        match self.eval_potential(e, at) {
            Ok(v) => Value::Known(v),
            Err(_) => Value::Unknown,
        }
    }

    /// Statement `s` executed with the parameters, locals and `this`
    /// unknown.
    fn nc_stmt(&mut self, s: StmtId, depth: u32) -> Flow {
        if depth > MAX_DEPTH {
            return Flow::Stop;
        }
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::NullStmt => Flow::Next,
            StmtKind::AttributedStmt => self.nc_stmt(st.sub_stmt(s), depth + 1),
            StmtKind::CompoundStmt => {
                let subs: Vec<StmtId> =
                    self.ast.lists.iter::<StmtId>(st.compound_body(s)).collect();
                for sub in subs {
                    match self.nc_stmt(sub, depth + 1) {
                        Flow::Next => {}
                        other => return other,
                    }
                }
                Flow::Next
            }
            StmtKind::ExprStmt => match self.nc_expr(st.expr_stmt_expr(s), depth + 1) {
                Value::Fails(fail) => Flow::Fails(fail),
                _ => Flow::Next,
            },
            StmtKind::ReturnStmt => match st.return_value(s).map(|v| self.nc_expr(v, depth + 1)) {
                Some(Value::Fails(fail)) => Flow::Fails(fail),
                _ => Flow::Stop,
            },
            StmtKind::DeclStmt => {
                let decls: Vec<DeclId> =
                    self.ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)).collect();
                for d in decls {
                    match self.kind_of(d) {
                        DeclKind::Var => {}
                        _ => continue,
                    }
                    if !self.has_local_storage(d) {
                        return Flow::Stop;
                    }
                    let init = self.ast.decls.var(d).and_then(|v| v.init);
                    if let Some(init) = init
                        && let Value::Fails(fail) = self.nc_expr(init, depth + 1)
                    {
                        return Flow::Fails(fail);
                    }
                }
                Flow::Next
            }
            StmtKind::IfStmt => {
                if !matches!(st.if_kind(s), IfKind::Ordinary | IfKind::Constexpr) {
                    return Flow::Stop;
                }
                let d = *st.if_data(s);
                if d.cond_var.is_some() {
                    return Flow::Stop;
                }
                if let Some(init) = d.init {
                    match self.nc_stmt(init, depth + 1) {
                        Flow::Next => {}
                        other => return other,
                    }
                }
                let Some(cond) = d.cond else { return Flow::Stop };
                match self.nc_expr(cond, depth + 1) {
                    Value::Fails(fail) => Flow::Fails(fail),
                    Value::Unknown => Flow::Stop,
                    Value::Known(v) => match self.const_truth(v) {
                        Some(b) => match if b { d.then } else { d.els } {
                            Some(branch) => self.nc_stmt(branch, depth + 1),
                            None => Flow::Next,
                        },
                        None => Flow::Stop,
                    },
                }
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                // The first iteration's body, when it is known to run;
                // what follows depends on how many iterations run.
                let do_loop = st.get(s).kind() == StmtKind::DoStmt;
                let d = *st.loop_data(s);
                if d.cond_var.is_some() {
                    return Flow::Stop;
                }
                if let Some(init) = d.init {
                    match self.nc_stmt(init, depth + 1) {
                        Flow::Next => {}
                        other => return other,
                    }
                }
                if !do_loop && let Some(cond) = d.cond {
                    match self.nc_expr(cond, depth + 1) {
                        Value::Fails(fail) => return Flow::Fails(fail),
                        Value::Known(v) if self.const_truth(v) == Some(true) => {}
                        _ => return Flow::Stop,
                    }
                }
                match d.body.map(|b| self.nc_stmt(b, depth + 1)) {
                    Some(Flow::Fails(fail)) => Flow::Fails(fail),
                    _ => Flow::Stop,
                }
            }
            // clang `EvaluateStmt`'s default: not valid (WP-915).
            StmtKind::GCCAsmStmt => Flow::Fails(Self::not_valid(st.get(s).loc())),
            _ => Flow::Stop,
        }
    }
}
