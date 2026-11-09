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
//! - any other function's body is walked as clang's `EvaluateStmt`
//!   executes it: a statement or initializer that names no parameter,
//!   local variable or `this` is evaluated on its own
//!   ([`Sema::eval_potential`]); one that does is taken apart operand by
//!   operand, as `enable_if` conditions are (`attr_cond.rs`).
//!
//! The walk mirrors clang's statement rules (WP-1133): an expression
//! statement (or `for` increment) whose value is unknown, or which may
//! call a function, notes a side effect and the walk goes on; from then
//! on destroying any local variable or temporary fails quietly
//! (`HandleDestruction`), and so does destroying one with a non-trivial
//! destructor at any time (the call is refused): the walk stops at the
//! end of such a block or full-expression ([`Walk`]). A discarded
//! expression that only names a parameter or local variable (`x;`,
//! `(void)x;`, `s.m;`, `&x;`) is evaluated as an lvalue and notes no side
//! effect (clang `EvaluateIgnoredValue`); one naming a reference
//! parameter or `this` does. Conditions must be known; loops run while
//! theirs is (a `break` ends one, a loop whose iteration repeats ends at
//! clang's step limit, `-fconstexpr-steps`, with clang's note at the
//! statement clang would be on); a `switch` runs its body from the
//! selected label, searching nested statements as clang does, and fails
//! when the label is inside a statement clang does not search (a
//! `try`-block, a statement expression: clang `EvaluateSwitch`'s
//! "statement expressions" note); a `try` runs its try-block; a `goto`
//! fails ("not valid"). The walk stops at a `return`, an `[[assume]]`, a
//! range-based `for`, a structured binding and an unknown value naming a
//! local variable it does not track. It fails at the definition of a
//! static or thread-local variable not usable in constant expressions and
//! at the end of a non-`void` function's body; a constructor's body is
//! walked only if each of its mem-initializers (default member
//! initializers included) is known.
//!
//! Local variables and condition variables have values, as in clang's
//! call frame (WP-1150): until the first side effect (after which clang
//! fails quietly to access any of them), a local variable of a type no
//! evaluation can leave a pointer in (arithmetic, enumeration, and arrays
//! and classes of those without a destructor) is a frame object
//! ([`kiln_oven::Oven::frame_local`]), initialized and later read and
//! modified by the Oven ([`Sema::eval_in_frame`]) with the parameters
//! still unknown; a reference variable bound to one names it. Any other
//! local variable is unknown, so the walk misses some functions clang
//! reports; it never reports one clang accepts. A loop whose iteration
//! leaves the frame as it found it repeats forever; any other runs
//! iteration by iteration.
//!
//! Builtin calls follow clang's table, never the Oven, which folds more
//! (the P0533 `<cmath>` builtins and their library twins) and notes some
//! calls its own way (WP-945; [`Sema::builtin_call_kind`] asks
//! `Sema::clang_constexpr_builtin`, `kiln_pp`'s `__has_constexpr_builtin`
//! for the target, `-std` and compat): a builtin clang does not evaluate
//! fails at its callee, another is opaque (its arguments walked), a
//! library builtin clang evaluates (`strlen`) always fails, any other
//! library builtin (`fabs`) fails once its arguments are known. A
//! function evaluated as a call first gets a walk in which only those
//! calls can fail. An `asm` statement fails; a function declared in a
//! system header is not checked.

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind};
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_oven::{EvalFailure, FailKind, FrameEval, FrameObject, Note};
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

/// A statement's outcome (clang's `EvalStmtResult`).
enum Flow {
    /// Execution goes on with the next statement (`ESR_Succeeded`).
    Next,
    /// The rest of the path is not known, or clang stops there quietly,
    /// or the function returned (`ESR_Failed`, `ESR_Returned`).
    Stop,
    /// It is never a constant.
    Fails(EvalFailure),
    /// A `break` (`ESR_Break`).
    Break,
    /// A `continue` (`ESR_Continue`).
    Continue,
    /// The `case` label sought was not found in it (`ESR_CaseNotFound`).
    CaseNotFound,
}

/// The most statements the walk executes before it gives up quietly
/// (its own budget: clang's `-fconstexpr-steps` limit is far larger).
const MAX_WORK: usize = 1 << 16;

/// The locals a block scope holds (clang `BlockScopeRAII`: every
/// local variable and lifetime-extended temporary registers a cleanup).
#[derive(Clone, Copy, Default)]
struct Scope {
    /// It holds one.
    locals: bool,
    /// One has a non-trivial destructor (whose call clang refuses).
    dtor: bool,
    /// The length of [`Walk::frame`] when it opened: its locals follow.
    mark: usize,
}

/// A local variable the walk knows the value of (WP-1150).
#[derive(Clone, Copy)]
struct Tracked {
    /// The variable.
    d: DeclId,
    /// Its object (a reference: its referent's).
    obj: FrameObject,
    /// The object is the variable's own (freed with it), not a
    /// referent's.
    own: bool,
}

/// The state of clang's evaluator along the walk.
struct Walk {
    /// clang's `EvalStatus.HasSideEffects`: a full-expression came out
    /// unknown (clang `noteSideEffect`/`noteFailure` and goes on), or
    /// may have done something clang does not model; from then on clang
    /// fails quietly to destroy any object (`HandleDestruction`).
    side: bool,
    /// The block scopes, innermost last.
    scopes: Vec<Scope>,
    /// clang `nextStep` calls so far (one per statement executed).
    steps: u64,
    /// The step limit (`-fconstexpr-steps`).
    limit: u64,
    /// The statements executed, in order.
    trace: Vec<StmtId>,
    /// A statement expression ran, whose statements clang counts as
    /// steps and the walk does not: no step-limit failure is reported.
    no_steps: bool,
    /// Local variables have values (the Oven is in, not the strict pass).
    track: bool,
    /// The local variables the walk knows the values of, in scope order.
    tracked: Vec<Tracked>,
    /// The frame `tracked` makes (each variable and its object).
    frame: Vec<(DeclId, FrameObject)>,
    /// Objects of variables whose scope ended, to free.
    dead: Vec<FrameObject>,
}

impl Walk {
    /// Opens a block scope.
    fn open(&mut self) {
        self.scopes.push(Scope { mark: self.tracked.len(), ..Scope::default() });
    }

    /// Ends the tracked variables of the block scope `s` closes.
    fn end_scope(&mut self, s: Scope) {
        if s.mark >= self.tracked.len() {
            return;
        }
        for t in self.tracked.drain(s.mark..) {
            if t.own {
                self.dead.push(t.obj);
            }
        }
        self.frame.truncate(s.mark);
    }

    /// Closes the innermost block scope without destroying its locals
    /// (clang `ScopeRAII::destroy(false)`).
    fn discard(&mut self) {
        let s = self.scopes.pop().unwrap_or_default();
        self.end_scope(s);
    }

    /// Closes the innermost block scope, destroying its locals: whether
    /// that succeeds (clang `ScopeRAII::destroy`).
    fn destroy(&mut self) -> bool {
        let s = self.scopes.pop().unwrap_or_default();
        self.end_scope(s);
        !(s.dtor || (self.side && s.locals))
    }

    /// Tracks local variable `d` (its object `obj`, its own if `own`).
    fn track_var(&mut self, d: DeclId, obj: FrameObject, own: bool) {
        self.tracked.push(Tracked { d, obj, own });
        self.frame.push((d, obj));
    }

    /// The object of tracked variable `d`.
    fn object_of(&self, d: DeclId) -> Option<FrameObject> {
        self.tracked.iter().rev().find(|t| t.d == d).map(|t| t.obj)
    }

    /// Whether the values of local variables are known: they are tracked
    /// and no side effect happened (from then on clang fails quietly to
    /// access any, C++14 `findCompleteObject`).
    fn knows_locals(&self) -> bool {
        self.track && !self.side
    }

    /// Closes the innermost block scope after its statement ended with
    /// `r`: clang destroys its locals unless it failed or a `case` label
    /// was not found, and a failed destruction fails the statement.
    fn close(&mut self, r: Flow) -> Flow {
        match r {
            Flow::Next | Flow::Break | Flow::Continue => {
                if self.destroy() {
                    r
                } else {
                    Flow::Stop
                }
            }
            _ => {
                self.discard();
                r
            }
        }
    }

    /// Registers a local (`dtor`: with a non-trivial destructor) in the
    /// innermost block scope.
    fn local(&mut self, dtor: bool) {
        if let Some(s) = self.scopes.last_mut() {
            s.locals = true;
            s.dtor |= dtor;
        }
    }
}

/// The temporaries of a full-expression clang registers cleanups for
/// ([`Sema::nc_temps`]).
#[derive(Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts about one full-expression")]
struct Temps {
    /// One destroyed at the end of the full-expression (a failure there
    /// fails the statement).
    any: bool,
    /// Such a one with a non-trivial destructor.
    dtor: bool,
    /// One destroyed at the end of an `ExprWithCleanups` (a failure
    /// there fails the expression: clang notes a side effect and goes
    /// on past an expression statement or initializer).
    ewc: bool,
    /// Such a one with a non-trivial destructor.
    ewc_dtor: bool,
    /// One lifetime-extended to the enclosing block.
    extended: bool,
    /// Such a one with a non-trivial destructor.
    extended_dtor: bool,
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
        // clang skips the (expensive) check for a function declared in a
        // system header, even under `-Wsystem-headers` (libstdc++'s
        // `<cmath>` overloads call builtins clang does not evaluate).
        if self.diags.sm().is_in_system_header(loc) {
            return;
        }
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
        let limit = self.oven.as_ref().map_or(1_048_576, |o| o.budget().steps);
        let mut w = Walk {
            side: false,
            scopes: Vec::new(),
            steps: 0,
            limit,
            trace: Vec::new(),
            no_steps: false,
        };
        match self.nc_stmt(&mut w, body, 0, None) {
            Flow::Fails(fail) => Some(fail),
            Flow::Next if self.nc_reaches_end_fails(f, body) => Some(EvalFailure {
                kind: FailKind::NotConstant,
                notes: vec![Note::new(
                    self.ast.stmts.rbrace(body),
                    "control reached end of constexpr function",
                )],
            }),
            _ => None,
        }
    }

    /// Whether the walk reaching the `}` of `f`'s `body` is a failure
    /// clang reports: a function returning non-`void` (clang
    /// `HandleFunctionCall` after its body succeeded).
    fn nc_reaches_end_fails(&self, f: DeclId, body: StmtId) -> bool {
        !self.nc_strict_only
            && self.ast.stmts.get(body).kind() == StmtKind::CompoundStmt
            && !matches!(self.kind_of(f), DeclKind::CXXConstructor | DeclKind::CXXDestructor)
            && {
                let ret = self.fn_ret(f);
                !self.is_error_type(ret)
                    && !self.ast.types.is_void(ret)
                    && !self.is_dependent_type(ret)
                    && !self.contains_undeduced_auto(ret)
            }
    }

    /// Whether evaluating `e` calls a function, or creates or destroys
    /// an object, in a way clang may refuse or not model (a nested call
    /// is never evaluated, clang `CheckCallLimit`): kinds outside a list
    /// of built-in operations, a call of a non-builtin function, a
    /// non-trivial constructor. Unevaluated operands do not count.
    fn nc_impure(&self, e: ExprId, depth: u32) -> bool {
        if depth > 256 {
            return true;
        }
        let ex = &self.ast.exprs;
        let mut ops = ex.children(e, &self.ast.lists);
        match ex.get(e).kind() {
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::TypeTraitExpr
            | ExprKind::ArrayTypeTraitExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::OffsetOfExpr
            | ExprKind::SizeOfPackExpr
            | ExprKind::RequiresExpr
            | ExprKind::ConceptSpecializationExpr
            | ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::ImaginaryLiteral
            | ExprKind::GNUNullExpr
            | ExprKind::PredefinedExpr
            | ExprKind::SourceLocExpr
            | ExprKind::DeclRefExpr
            | ExprKind::CXXThisExpr
            | ExprKind::ImplicitValueInitExpr
            | ExprKind::CXXScalarValueInitExpr
            | ExprKind::ArrayInitIndexExpr => return false,
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr => {
                let Some((callee, f)) = self.nc_direct_callee(e) else { return true };
                if self.ast.decls.builtin_id(f).is_none() {
                    return true;
                }
                ops.retain(|&c| c != callee);
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                if !self.fn_spec(ex.constructor(e)).is_trivial() {
                    return true;
                }
            }
            ExprKind::MemberExpr
            | ExprKind::UnaryOperator
            | ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator
            | ExprKind::ConditionalOperator
            | ExprKind::ArraySubscriptExpr
            | ExprKind::ParenExpr
            | ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::InitListExpr
            | ExprKind::CXXParenListInitExpr
            | ExprKind::ArrayInitLoopExpr
            | ExprKind::OpaqueValueExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::ConstantExpr => {}
            // Anything else may call a function or write memory (a
            // default member initializer may call anything).
            _ => return true,
        }
        ops.into_iter().any(|c| self.nc_impure(c, depth + 1))
    }

    /// Whether `e` contains a statement expression (outside lambda
    /// bodies and unevaluated operands).
    fn nc_has_stmt_expr(&self, e: ExprId, depth: u32) -> bool {
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::StmtExpr => true,
            ExprKind::LambdaExpr
            | ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::RequiresExpr => false,
            _ if depth > 256 => true,
            _ => ex
                .children(e, &self.ast.lists)
                .into_iter()
                .any(|c| self.nc_has_stmt_expr(c, depth + 1)),
        }
    }

    /// Records the full-expression `e`, whose value came out `v`, in
    /// `w`: whether the walk can go on past it. An unknown value (clang
    /// fails, notes a side effect and goes on) or a possible side effect
    /// ([`Sema::nc_impure`], an unknown name) sets [`Walk::side`]; kiln
    /// sets it in more cases than clang, which only makes the walk stop
    /// sooner. An unknown value naming a local variable, whose value
    /// clang knows (and may fail on), ends the walk.
    fn nc_full_expr(&self, w: &mut Walk, e: ExprId, v: &Value) -> bool {
        if self.nc_has_stmt_expr(e, 0) {
            w.no_steps = true;
        }
        let known = matches!(v, Value::Known(_));
        if !known || self.names_unknown(e, 0) || self.nc_impure(e, 0) {
            w.side = true;
        }
        known || !self.names_local(e, 0)
    }

    /// Whether `e` names a local variable other than a parameter.
    fn names_local(&self, e: ExprId, depth: u32) -> bool {
        if depth > 256 {
            return true;
        }
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::DeclRefExpr => {
                let d = ex.decl(e);
                self.kind_of(d) != DeclKind::ParmVar && self.has_local_storage(d)
            }
            _ => {
                ex.children(e, &self.ast.lists).into_iter().any(|c| self.names_local(c, depth + 1))
            }
        }
    }

    /// The temporaries of `e` clang registers cleanups for, added to
    /// `out`: a materialized temporary (one not lifetime-extended is
    /// destroyed by the innermost enclosing `ExprWithCleanups`, if any:
    /// `in_ewc`), and a class or array prvalue whose value is discarded
    /// (`ignored`: the full-expression of an expression statement, the
    /// operand of a cast to `void`, the left operand of a comma; clang's
    /// `Evaluate` makes it a temporary before visiting it).
    fn nc_temps(&self, e: ExprId, ignored: bool, in_ewc: bool, out: &mut Temps, depth: u32) {
        if depth > 256 {
            out.any = true;
            out.dtor = true;
            return;
        }
        let ex = &self.ast.exprs;
        let n = ex.get(e);
        let ty = n.ty();
        let kind = n.kind();
        let note = |out: &mut Temps, in_ewc: bool, dtor: bool| {
            if in_ewc {
                out.ewc = true;
                out.ewc_dtor |= dtor;
            } else {
                out.any = true;
                out.dtor |= dtor;
            }
        };
        if ignored
            && !n.value_category().is_glvalue()
            && (self.ast.types.as_record(ty).is_some() || self.ast.types.is_array(ty))
        {
            note(out, in_ewc, self.nc_type_destroyed(ty));
        }
        let in_ewc = in_ewc || kind == ExprKind::ExprWithCleanups;
        let ops = ex.children(e, &self.ast.lists);
        match kind {
            ExprKind::LambdaExpr
            | ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::CXXTypeidExpr
            | ExprKind::RequiresExpr => {}
            ExprKind::MaterializeTemporaryExpr => {
                let sub = ops.first().copied();
                let dtor = self.nc_type_destroyed(sub.map_or(ty, |s| ex.get(s).ty()));
                match ex.temporary_storage(e) {
                    kiln_ast::expr::StorageDuration::FullExpression => note(out, in_ewc, dtor),
                    kiln_ast::expr::StorageDuration::Automatic => {
                        out.extended = true;
                        out.extended_dtor |= dtor;
                    }
                    _ => {}
                }
                for c in ops {
                    self.nc_temps(c, false, in_ewc, out, depth + 1);
                }
            }
            ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXFunctionalCastExpr
                if ex.cast_kind(e) == kiln_ast::expr::CastKind::ToVoid =>
            {
                for c in ops {
                    self.nc_temps(c, true, in_ewc, out, depth + 1);
                }
            }
            ExprKind::ParenExpr => {
                for c in ops {
                    self.nc_temps(c, ignored, in_ewc, out, depth + 1);
                }
            }
            ExprKind::BinaryOperator if ex.binary_op(e) == BinaryOp::Comma => {
                let [l, r] = ex.operands(e);
                self.nc_temps(l, true, in_ewc, out, depth + 1);
                self.nc_temps(r, ignored, in_ewc, out, depth + 1);
            }
            ExprKind::ConditionalOperator => {
                let [t, f] = ex.cond_arms(e);
                for c in ops {
                    self.nc_temps(c, ignored && (c == t || c == f), in_ewc, out, depth + 1);
                }
            }
            _ => {
                for c in ops {
                    self.nc_temps(c, false, in_ewc, out, depth + 1);
                }
            }
        }
    }

    /// Records in `w` the temporaries `t` of a full-expression whose
    /// failures clang notes as side effects and goes on past (an
    /// expression statement, an initializer): whether destroying those
    /// at its end succeeds.
    fn nc_temps_destroyed(w: &mut Walk, t: Temps) -> bool {
        if t.ewc_dtor {
            w.side = true;
        }
        !(t.dtor || (w.side && t.any))
    }

    /// The expression statement or `for` increment `e`, its value
    /// discarded (clang `EvaluateIgnoredValue` in a full-expression
    /// scope): it fails, or clang goes on unless destroying a temporary
    /// fails ([`Walk::destroy`]).
    fn nc_ignored(&mut self, w: &mut Walk, e: ExprId, depth: u32) -> Flow {
        let v = self.nc_expr(e, depth + 1);
        if let Value::Fails(fail) = v {
            return Flow::Fails(fail);
        }
        if !self.nc_full_expr(w, e, &v) {
            return Flow::Stop;
        }
        let mut t = Temps::default();
        self.nc_temps(e, true, false, &mut t, 0);
        if Self::nc_temps_destroyed(w, t) { Flow::Next } else { Flow::Stop }
    }

    /// A condition (clang `EvaluateCond` or `EvaluateInteger` in a
    /// full-expression scope): its value when known and its temporaries
    /// are destroyed, else how the walk ends.
    fn nc_cond(&mut self, w: &mut Walk, c: ExprId, depth: u32) -> Result<ConstValueId, Flow> {
        let v = self.nc_expr(c, depth + 1);
        // An unknown value stops the walk here anyway.
        let _ = self.nc_full_expr(w, c, &v);
        let v = match v {
            Value::Fails(fail) => return Err(Flow::Fails(fail)),
            Value::Unknown => return Err(Flow::Stop),
            Value::Known(v) => v,
        };
        let mut t = Temps::default();
        self.nc_temps(c, false, false, &mut t, 0);
        if t.dtor || t.ewc_dtor || t.extended_dtor || (w.side && (t.any || t.ewc || t.extended)) {
            return Err(Flow::Stop);
        }
        Ok(v)
    }

    /// clang's failure at the definition of static or thread-local local
    /// variable `d` not usable in constant expressions
    /// (`CheckLocalVariableDeclaration`).
    fn nc_static_local(&mut self, d: DeclId) -> Option<EvalFailure> {
        let v = self.ast.decls.var(d)?;
        let tls = v.spec.tls() != kiln_ast::decl::TlsKind::None;
        if (!tls && v.spec.storage() != kiln_ast::decl::VarStorage::Static)
            || self.usable_in_constant_expressions(d)
        {
            return None;
        }
        let what = if tls { "thread_local" } else { "static" };
        Some(EvalFailure {
            kind: FailKind::NotConstant,
            notes: vec![Note::new(
                self.ast.decls.get(d).loc(),
                format!("control flows through the definition of a {what} variable"),
            )],
        })
    }

    /// Whether an object of type `ty` is destroyed by a non-trivial
    /// destructor call (which clang refuses).
    fn nc_type_destroyed(&self, ty: QualType) -> bool {
        let elem = self.base_element(ty);
        self.ast
            .types
            .as_record(elem)
            .is_some_and(|c| self.ast.destructor(c).is_some_and(|x| !self.fn_spec(x).is_trivial()))
    }

    /// Whether local variable `d`'s lifetime ends in a destructor call
    /// the walk does not evaluate.
    fn nc_destroyed(&self, d: DeclId) -> bool {
        self.nc_type_destroyed(self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR))
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
            return self.nc_strict_value(e, evaluable);
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
                    if let Some(v) = self.nc_builtin_call(e, callee, f, &ops, depth) {
                        return v;
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

    /// The call `e` of `f` through `callee` with arguments `ops`, when `f`
    /// is a builtin ([`Sema::builtin_call_kind`]); `None` for any other
    /// function.
    fn nc_builtin_call(
        &mut self,
        e: ExprId,
        callee: ExprId,
        f: DeclId,
        ops: &[ExprId],
        depth: u32,
    ) -> Option<Value> {
        Some(match self.builtin_call_kind(f) {
            BuiltinCall::None => return None,
            BuiltinCall::Invalid => Value::Fails(Self::not_valid(self.begin_loc(callee))),
            BuiltinCall::LibEvaluated(name) => Value::Fails(EvalFailure {
                kind: FailKind::NotConstant,
                notes: vec![Self::non_constexpr_note(self.begin_loc(e), name)],
            }),
            BuiltinCall::LibPlain => self.nc_lib_call(e, f, ops.to_vec(), depth),
            BuiltinCall::Opaque => {
                for c in self.opaque_builtin_operands(f, ops.to_vec()) {
                    if let p @ Value::Fails(_) = self.nc_expr(c, depth + 1) {
                        return Some(p);
                    }
                }
                Value::Unknown
            }
        })
    }

    /// `e`, calling no builtin of [`BuiltinCalls::strict`] kind, in the
    /// strict pass: only such a call may fail there, so `e` contributes
    /// its value when the Oven has one (`fabs(-2.0)`) and is otherwise
    /// unknown.
    fn nc_strict_value(&mut self, e: ExprId, evaluable: bool) -> Value {
        if !evaluable || self.names_unknown(e, 0) {
            return Value::Unknown;
        }
        match self.nc_eval(e) {
            v @ Value::Known(_) => v,
            _ => Value::Unknown,
        }
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

    /// The failure at statement `s` once the walk has used up clang's
    /// step budget (clang `nextStep`), unless the walk cannot count
    /// clang's steps.
    fn nc_step_fail(&self, w: &Walk, s: StmtId) -> Flow {
        if self.nc_strict_only || w.no_steps {
            return Flow::Stop;
        }
        Flow::Fails(EvalFailure {
            kind: FailKind::NotConstant,
            notes: vec![Note::new(
                self.stmt_begin_loc(s),
                "constexpr evaluation hit maximum step limit; possible infinite loop?",
            )],
        })
    }

    /// A loop iteration that began at `w.trace[start]`, with
    /// [`Walk::side`] the same at its end as at its start, has finished
    /// and the loop goes on: every later iteration executes the same
    /// statements (the walk's values do not depend on the iteration), so
    /// clang runs out of steps at a statement of that sequence.
    fn nc_cycle(&self, w: &Walk, start: usize) -> Flow {
        let seq = &w.trace[start..];
        if seq.is_empty() || self.nc_strict_only || w.no_steps {
            return Flow::Stop;
        }
        let k = (w.limit.saturating_sub(w.steps) % seq.len() as u64) as usize;
        self.nc_step_fail(w, seq[k])
    }

    /// A loop body (clang `EvaluateLoopBody`): a block scope of its own;
    /// `break` ends the loop ([`Flow::Next`]), the end of the body or a
    /// `continue` goes on with it ([`Flow::Continue`]).
    fn nc_loop_body(
        &mut self,
        w: &mut Walk,
        body: StmtId,
        depth: u32,
        case: Option<StmtId>,
    ) -> Flow {
        w.open();
        let r = self.nc_stmt(w, body, depth + 1, case);
        match w.close(r) {
            Flow::Break => Flow::Next,
            Flow::Next | Flow::Continue => Flow::Continue,
            r => r,
        }
    }

    /// A `while` loop from its condition.
    fn nc_while(&mut self, w: &mut Walk, s: StmtId, depth: u32) -> Flow {
        let d = *self.ast.stmts.loop_data(s);
        let (None, Some(cond), Some(body)) = (d.cond_var, d.cond, d.body) else {
            return Flow::Stop;
        };
        loop {
            let (side, start) = (w.side, w.trace.len());
            let v = match self.nc_cond(w, cond, depth) {
                Ok(v) => v,
                Err(r) => return r,
            };
            match self.const_truth(v) {
                Some(true) => {}
                Some(false) => return Flow::Next,
                None => return Flow::Stop,
            }
            match self.nc_loop_body(w, body, depth, None) {
                Flow::Continue => {}
                r => return r,
            }
            if side == w.side {
                return self.nc_cycle(w, start);
            }
        }
    }

    /// A `do` loop, entered looking for `case` label `case`.
    fn nc_do(&mut self, w: &mut Walk, s: StmtId, depth: u32, mut case: Option<StmtId>) -> Flow {
        let d = *self.ast.stmts.loop_data(s);
        let (Some(cond), Some(body)) = (d.cond, d.body) else { return Flow::Stop };
        loop {
            let (side, start, whole) = (w.side, w.trace.len(), case.is_none());
            match self.nc_loop_body(w, body, depth, case) {
                Flow::Continue => {}
                r => return r,
            }
            case = None;
            let v = match self.nc_cond(w, cond, depth) {
                Ok(v) => v,
                Err(r) => return r,
            };
            match self.const_truth(v) {
                Some(true) => {}
                Some(false) => return Flow::Next,
                None => return Flow::Stop,
            }
            if whole && side == w.side {
                return self.nc_cycle(w, start);
            }
        }
    }

    /// A `for` loop (its init statement and condition run in a block
    /// scope of their own).
    fn nc_for(&mut self, w: &mut Walk, s: StmtId, depth: u32) -> Flow {
        let d = *self.ast.stmts.loop_data(s);
        let (None, Some(body)) = (d.cond_var, d.body) else { return Flow::Stop };
        w.open();
        if let Some(init) = d.init {
            match self.nc_stmt(w, init, depth + 1, None) {
                Flow::Next => {}
                r => return w.close(r),
            }
        }
        loop {
            let (side, start) = (w.side, w.trace.len());
            if let Some(cond) = d.cond {
                let v = match self.nc_cond(w, cond, depth) {
                    Ok(v) => v,
                    Err(r) => return w.close(r),
                };
                match self.const_truth(v) {
                    Some(true) => {}
                    Some(false) => return w.close(Flow::Next),
                    None => return w.close(Flow::Stop),
                }
            }
            match self.nc_loop_body(w, body, depth, None) {
                Flow::Continue => {}
                r => return w.close(r),
            }
            if let Some(inc) = d.inc {
                match self.nc_ignored(w, inc, depth) {
                    Flow::Next => {}
                    r => return w.close(r),
                }
            }
            if side == w.side {
                w.discard();
                return self.nc_cycle(w, start);
            }
        }
    }

    /// A `for` loop searched for `case` label `case`: on finding it in
    /// the body, the loop goes on from its increment (clang runs the
    /// init statement again).
    fn nc_for_case(&mut self, w: &mut Walk, s: StmtId, depth: u32, case: StmtId) -> Flow {
        let d = *self.ast.stmts.loop_data(s);
        let (None, Some(body)) = (d.cond_var, d.body) else { return Flow::Stop };
        w.open();
        if let Some(init) = d.init {
            match self.nc_stmt(w, init, depth + 1, Some(case)) {
                Flow::CaseNotFound => {}
                r => {
                    w.discard();
                    return r;
                }
            }
        }
        match self.nc_loop_body(w, body, depth, Some(case)) {
            Flow::Continue => {}
            r => {
                w.discard();
                return r;
            }
        }
        if let Some(inc) = d.inc {
            match self.nc_ignored(w, inc, depth) {
                Flow::Next => {}
                r => {
                    w.discard();
                    return r;
                }
            }
        }
        w.discard();
        self.nc_for(w, s, depth)
    }

    /// An `if` statement.
    fn nc_if(&mut self, w: &mut Walk, s: StmtId, depth: u32) -> Flow {
        let st = &self.ast.stmts;
        if !matches!(st.if_kind(s), IfKind::Ordinary | IfKind::Constexpr) {
            return Flow::Stop;
        }
        let d = *st.if_data(s);
        let (None, Some(cond)) = (d.cond_var, d.cond) else { return Flow::Stop };
        w.open();
        if let Some(init) = d.init {
            match self.nc_stmt(w, init, depth + 1, None) {
                Flow::Next => {}
                r => return w.close(r),
            }
        }
        let v = match self.nc_cond(w, cond, depth) {
            Ok(v) => v,
            Err(r) => return w.close(r),
        };
        let Some(b) = self.const_truth(v) else { return w.close(Flow::Stop) };
        let r = match if b { d.then } else { d.els } {
            Some(branch) => self.nc_stmt(w, branch, depth + 1, None),
            None => Flow::Next,
        };
        w.close(r)
    }

    /// An `if` statement searched for `case` label `case` (in its init
    /// statement, then its branches in turn).
    fn nc_if_case(&mut self, w: &mut Walk, s: StmtId, depth: u32, case: StmtId) -> Flow {
        let st = &self.ast.stmts;
        if !matches!(st.if_kind(s), IfKind::Ordinary | IfKind::Constexpr) {
            return Flow::Stop;
        }
        let d = *st.if_data(s);
        w.open();
        if let Some(init) = d.init {
            match self.nc_stmt(w, init, depth + 1, Some(case)) {
                Flow::CaseNotFound => {}
                r => {
                    w.discard();
                    return r;
                }
            }
        }
        for branch in [d.then, d.els].into_iter().flatten() {
            match self.nc_stmt(w, branch, depth + 1, Some(case)) {
                Flow::CaseNotFound => {}
                r => return w.close(r),
            }
        }
        w.discard();
        Flow::CaseNotFound
    }

    /// A `switch` statement: the body runs from the `case` label its
    /// condition selects.
    fn nc_switch(&mut self, w: &mut Walk, s: StmtId, depth: u32) -> Flow {
        let d = *self.ast.stmts.switch_data(s);
        if d.cond_var.is_some() {
            return Flow::Stop;
        }
        w.open();
        if let Some(init) = d.init {
            match self.nc_stmt(w, init, depth + 1, None) {
                Flow::Next => {}
                r => return w.close(r),
            }
        }
        let v = match self.nc_cond(w, d.cond, depth) {
            Ok(v) => v,
            Err(r) => return w.close(r),
        };
        let Some(v) = self.const_int(v) else { return w.close(Flow::Stop) };
        let cases: Vec<StmtId> = self.ast.lists.iter::<StmtId>(d.cases).collect();
        let (mut default, mut found) = (None, None);
        for c in cases {
            if self.ast.stmts.get(c).kind() == StmtKind::DefaultStmt {
                default = Some(c);
                continue;
            }
            let cd = *self.ast.stmts.case_data(c);
            let Some(lo) = self.nc_case_value(cd.lhs) else { return w.close(Flow::Stop) };
            let hi = match cd.rhs {
                Some(rhs) => match self.nc_case_value(rhs) {
                    Some(hi) => hi,
                    None => return w.close(Flow::Stop),
                },
                None => lo,
            };
            if lo <= v && v <= hi {
                found = Some(c);
                break;
            }
        }
        let Some(found) = found.or(default) else { return w.close(Flow::Next) };
        let r = self.nc_stmt(w, d.body, depth + 1, Some(found));
        match w.close(r) {
            Flow::Break => Flow::Next,
            // clang fails ("statement expression not supported") at a
            // label it cannot reach; kiln does not mirror that note.
            Flow::CaseNotFound => Flow::Stop,
            r => r,
        }
    }

    /// The value of `case` label expression `e`.
    fn nc_case_value(&mut self, e: ExprId) -> Option<i128> {
        if self.ast.exprs.get(e).kind() != ExprKind::ConstantExpr {
            return None;
        }
        self.fold_int(e)
    }

    /// A declaration statement: each variable is defined in turn (a
    /// static or thread-local one is checked; a local one is registered
    /// in the block scope, then initialized as a full-expression).
    fn nc_decl_stmt(&mut self, w: &mut Walk, s: StmtId, depth: u32) -> Flow {
        let decls: Vec<DeclId> =
            self.ast.lists.iter::<DeclId>(self.ast.stmts.decl_stmt_decls(s)).collect();
        for d in decls {
            match self.nc_local_decl(w, d) {
                Some(Flow::Next) => {}
                Some(r) => return r,
                None => continue,
            }
            let Some(init) = self.ast.decls.var(d).and_then(|v| v.init) else { continue };
            let v = self.nc_expr(init, depth + 1);
            if let Value::Fails(fail) = v {
                return Flow::Fails(fail);
            }
            if !self.nc_full_expr(w, init, &v) {
                return Flow::Stop;
            }
            let mut t = Temps::default();
            self.nc_temps(init, false, false, &mut t, 0);
            if t.extended {
                w.local(t.extended_dtor);
            }
            if !Self::nc_temps_destroyed(w, t) {
                return Flow::Stop;
            }
        }
        Flow::Next
    }

    /// A declaration statement searched for a `case` label: clang checks
    /// each static variable and starts the lifetime of each local one
    /// without an initializer.
    fn nc_decl_case(&mut self, w: &mut Walk, s: StmtId) -> Flow {
        let decls: Vec<DeclId> =
            self.ast.lists.iter::<DeclId>(self.ast.stmts.decl_stmt_decls(s)).collect();
        for d in decls {
            let init = self.ast.decls.var(d).and_then(|v| v.init);
            if init.is_some() && self.kind_of(d) == DeclKind::Var && self.has_local_storage(d) {
                if self.ast.decls.decomp_bindings(d).is_some() {
                    return Flow::Stop;
                }
                continue;
            }
            match self.nc_local_decl(w, d) {
                Some(Flow::Next) | None => {}
                Some(r) => return r,
            }
        }
        Flow::CaseNotFound
    }

    /// The definition of `d` up to its initializer: `None` if it defines
    /// nothing the walk tracks (not a variable, a block-scope `extern`, a
    /// static variable usable in constant expressions), else how the
    /// walk goes on ([`Flow::Next`]: a local variable, now registered).
    fn nc_local_decl(&mut self, w: &mut Walk, d: DeclId) -> Option<Flow> {
        if self.kind_of(d) != DeclKind::Var
            || self
                .ast
                .decls
                .var(d)
                .is_some_and(|v| v.spec.storage() == kiln_ast::decl::VarStorage::Extern)
        {
            return None;
        }
        if self.ast.decls.decomp_bindings(d).is_some() {
            return Some(Flow::Stop);
        }
        if !self.has_local_storage(d) {
            // A static or thread-local variable fails before its
            // initializer runs (clang `CheckLocalVariableDeclaration`).
            if self.nc_strict_only {
                return Some(Flow::Stop);
            }
            return self.nc_static_local(d).map(Flow::Fails);
        }
        w.local(self.nc_destroyed(d));
        Some(Flow::Next)
    }

    /// Statement `s` executed with the parameters, locals and `this`
    /// unknown (clang `EvaluateStmt`), searching for `case` label
    /// `case` when it is set.
    fn nc_stmt(&mut self, w: &mut Walk, s: StmtId, depth: u32, case: Option<StmtId>) -> Flow {
        if depth > MAX_DEPTH || w.trace.len() >= MAX_WORK {
            return Flow::Stop;
        }
        if w.steps >= w.limit {
            return self.nc_step_fail(w, s);
        }
        w.steps += 1;
        w.trace.push(s);
        let kind = self.ast.stmts.get(s).kind();
        let mut case = case;
        if let Some(c) = case {
            match kind {
                StmtKind::CompoundStmt
                | StmtKind::LabelStmt
                | StmtKind::AttributedStmt
                | StmtKind::DoStmt => {}
                StmtKind::CaseStmt | StmtKind::DefaultStmt => {
                    if c == s {
                        case = None;
                    }
                }
                StmtKind::IfStmt => return self.nc_if_case(w, s, depth, c),
                StmtKind::WhileStmt => {
                    let d = *self.ast.stmts.loop_data(s);
                    let (None, Some(body)) = (d.cond_var, d.body) else { return Flow::Stop };
                    return match self.nc_loop_body(w, body, depth, Some(c)) {
                        Flow::Continue => self.nc_while(w, s, depth),
                        r => r,
                    };
                }
                StmtKind::ForStmt => return self.nc_for_case(w, s, depth, c),
                StmtKind::DeclStmt => return self.nc_decl_case(w, s),
                _ => return Flow::CaseNotFound,
            }
        }
        let st = &self.ast.stmts;
        match kind {
            StmtKind::NullStmt => Flow::Next,
            // An assumption is evaluated (and fails quietly when it
            // names an unknown); the walk stops there.
            StmtKind::AttributedStmt
                if self
                    .ast
                    .lists
                    .iter::<kiln_ast::AttrId>(st.stmt_attrs(s))
                    .any(|a| self.ast.attrs.get(a).kind == kiln_ast::attr::AttrKind::ASSUME) =>
            {
                Flow::Stop
            }
            StmtKind::AttributedStmt | StmtKind::LabelStmt => {
                self.nc_stmt(w, st.sub_stmt(s), depth + 1, case)
            }
            StmtKind::CaseStmt => self.nc_stmt(w, st.case_data(s).sub, depth + 1, case),
            StmtKind::DefaultStmt => self.nc_stmt(w, st.default_sub(s), depth + 1, case),
            // clang evaluates the try-block alone.
            StmtKind::CXXTryStmt => self.nc_stmt(w, st.try_block(s), depth + 1, case),
            StmtKind::CompoundStmt => {
                let subs: Vec<StmtId> =
                    self.ast.lists.iter::<StmtId>(st.compound_body(s)).collect();
                w.open();
                for sub in subs {
                    match self.nc_stmt(w, sub, depth + 1, case) {
                        Flow::Next => case = None,
                        Flow::CaseNotFound => {}
                        r => return w.close(r),
                    }
                }
                if case.is_some() {
                    w.discard();
                    Flow::CaseNotFound
                } else {
                    w.close(Flow::Next)
                }
            }
            StmtKind::ExprStmt => self.nc_ignored(w, st.expr_stmt_expr(s), depth),
            StmtKind::ReturnStmt => match st.return_value(s).map(|v| self.nc_expr(v, depth + 1)) {
                Some(Value::Fails(fail)) => Flow::Fails(fail),
                _ => Flow::Stop,
            },
            StmtKind::DeclStmt => self.nc_decl_stmt(w, s, depth),
            StmtKind::IfStmt => self.nc_if(w, s, depth),
            StmtKind::WhileStmt => self.nc_while(w, s, depth),
            StmtKind::ForStmt => self.nc_for(w, s, depth),
            StmtKind::DoStmt => self.nc_do(w, s, depth, case),
            StmtKind::SwitchStmt => self.nc_switch(w, s, depth),
            StmtKind::BreakStmt => Flow::Break,
            StmtKind::ContinueStmt => Flow::Continue,
            // clang `EvaluateStmt`'s default: not valid (WP-915).
            StmtKind::GotoStmt | StmtKind::IndirectGotoStmt => {
                Flow::Fails(Self::not_valid(self.stmt_begin_loc(s)))
            }
            StmtKind::GCCAsmStmt => Flow::Fails(Self::not_valid(st.get(s).loc())),
            _ => Flow::Stop,
        }
    }
}
