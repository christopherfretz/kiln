//! `-Wreturn-stack-address`: a returned pointer, reference or closure that
//! refers to an automatic variable or parameter of the returning function
//! ([basic.stc.auto], [stmt.return]; clang's `checkExprLifetime` for a
//! return, spec/sema-class.md §5.6).
//!
//! The returned value is walked the way clang's
//! `visitLocalsRetainedByInitializer` (a pointer or class prvalue) and
//! `visitLocalsRetainedByReferenceBinding` (a glvalue bound by a returned
//! reference) do, recording the path of `&`, array decay, reference and
//! `const` variable initializers and lambda captures that leads to each
//! local. Each local found is diagnosed at the first variable or capture
//! on its path (else at the local itself), followed by one note per
//! variable or capture. A temporary reached through a variable's
//! initializer is the `returning reference to local temporary object`
//! error; one bound directly is diagnosed by
//! [`Sema::check_dangling_temporary`].

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, StorageDuration, UnaryOp};
use kiln_ast::lambda::{CaptureKind, LambdaCapture};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// The recursion bound of the walk.
const MAX_DEPTH: u32 = 256;

/// One step from the returned expression to a local (clang's
/// `IndirectLocalPathEntry`).
#[derive(Copy, Clone)]
enum Step {
    /// `&e` or the array-to-pointer decay of `e` (no note).
    AddressOf,
    /// Through the initializer of the reference or `const` variable
    /// `var`, named at `at`.
    VarInit { var: DeclId, at: Loc },
    /// Through the initializer of a lambda capture, written (or implied)
    /// at `at`.
    Capture { cap: LambdaCapture, at: Loc },
}

/// What a returned value refers to.
#[derive(Copy, Clone)]
enum Local {
    /// A `DeclRefExpr` naming an automatic variable or parameter.
    Var(ExprId),
    /// A `MaterializeTemporaryExpr`.
    Temp(ExprId),
}

/// The walk's state: the current path and the locals found.
#[derive(Default)]
struct Walk {
    path: Vec<Step>,
    found: Vec<(Local, Vec<Step>)>,
}

impl Walk {
    fn record(&mut self, l: Local) {
        self.found.push((l, self.path.clone()));
    }

    fn on_path(&self, v: DeclId) -> bool {
        self.path.iter().any(|s| matches!(s, Step::VarInit { var, .. } if *var == v))
    }
}

impl Sema<'_, '_> {
    /// The lifetime diagnostics of `return c;` in a function returning
    /// `ret` (`c` is the converted operand).
    pub(crate) fn check_return_lifetime(&mut self, c: ExprId, ret: QualType) {
        let by_ref = self.ast.types.is_reference(ret);
        if by_ref {
            self.check_dangling_temporary(super::BindingSite::Return, c);
        }
        if self.expr_is_type_dependent(c) {
            return;
        }
        let mut w = Walk::default();
        if by_ref {
            self.retained_by_ref(c, &mut w, 0);
        } else {
            self.retained_by_value(c, &mut w, 0);
        }
        for (local, path) in w.found {
            self.diagnose_returned_local(local, &path, by_ref);
        }
    }

    /// `-Wdangling` for the pointer variable `v` initialized by `init`:
    /// the address of a temporary (an array decayed, directly or through
    /// a member or a conditional arm) dangles once the full-expression
    /// ends ([class.temporary]/4; clang's `checkExprLifetime` for an
    /// `EK_Variable` entity, WP-227). The reference forms and pointers
    /// reached through another variable's initializer are not diagnosed
    /// yet (spec/sema-class.md §5.6).
    pub(crate) fn check_dangling_pointer_var(&mut self, v: DeclId, init: ExprId) {
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        if !self.ast.types.is_pointer(ty) || self.expr_is_type_dependent(init) {
            return;
        }
        // `p{e}`: the list's element initialized the pointer.
        let ex = &self.ast.exprs;
        let mut e = init;
        if ex.get(e).kind() == ExprKind::ExprWithCleanups {
            e = ex.operand(e);
        }
        if ex.get(e).kind() == ExprKind::InitListExpr {
            let mut inits = self.ast.lists.iter::<ExprId>(ex.init_list_inits(e));
            match (inits.next(), inits.next()) {
                (Some(x), None) => e = x,
                _ => return,
            }
        }
        let mut w = Walk::default();
        self.retained_by_value(e, &mut w, 0);
        for (local, path) in w.found {
            let Local::Temp(mte) = local else { continue };
            if !path.iter().all(|s| matches!(s, Step::AddressOf)) {
                continue;
            }
            let msg = self
                .diag(DiagId::sema_dangling_variable, self.begin_loc(mte))
                .arg("0")
                .arg(self.decl_arg(v));
            self.emit(msg);
        }
    }

    fn diagnose_returned_local(&mut self, local: Local, path: &[Step], by_ref: bool) {
        let first = path.iter().find_map(|s| match *s {
            Step::VarInit { at, .. } | Step::Capture { at, .. } => Some(at),
            Step::AddressOf => None,
        });
        let mut msg = match local {
            Local::Var(dre) => {
                let d = self.ast.exprs.decl(dre);
                let loc = first.unwrap_or_else(|| self.begin_loc(dre));
                let is_param = self.kind_of(d) == DeclKind::ParmVar;
                self.diag(DiagId::sema_ret_stack_addr, loc)
                    .arg(u8::from(by_ref))
                    .arg(u8::from(is_param))
                    .arg(self.decl_arg(d))
            }
            // A temporary bound directly is `check_dangling_temporary`'s.
            Local::Temp(_) if first.is_none() => return,
            Local::Temp(_) => {
                let id = if by_ref {
                    DiagId::sema_temp_return_local
                } else {
                    DiagId::sema_return_local_temp_addr
                };
                self.diag(id, first.unwrap_or(Loc::INVALID))
            }
        };
        for s in path {
            match *s {
                Step::AddressOf => {}
                Step::VarInit { var, .. } => {
                    let d = self.ast.decls.get(var);
                    let is_ref = self.ast.types.is_reference(d.ty().unwrap_or(QualType::ERROR));
                    let note = self
                        .diag(DiagId::note_local_var_init, d.loc())
                        .arg(u8::from(is_ref))
                        .arg(self.decl_arg(var));
                    msg = msg.with_note(note);
                }
                Step::Capture { cap, .. } => {
                    let by_ref = cap.kind == CaptureKind::ByRef;
                    let note = if cap.init {
                        let Some(v) = cap.var else { continue };
                        self.diag(DiagId::note_lambda_init_capture, cap.loc)
                            .arg(u8::from(by_ref))
                            .arg(self.decl_arg(v))
                    } else {
                        self.diag(DiagId::note_lambda_capture_here, cap.loc)
                            .arg(u8::from(by_ref) + 2 * u8::from(cap.implicit))
                    };
                    msg = msg.with_note(note);
                }
            }
        }
        self.emit(msg);
    }

    /// Whether `d` is a parameter or a variable with automatic storage
    /// duration of the enclosing function.
    fn is_automatic_local(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::ParmVar => true,
            DeclKind::Var => self.var_storage(d) == StorageDuration::Automatic,
            _ => false,
        }
    }

    /// The locals the pointer, closure or other prvalue `e` refers to
    /// (clang's `visitLocalsRetainedByInitializer`).
    fn retained_by_value(&self, e: ExprId, w: &mut Walk, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        let n = *self.ast.exprs.get(e);
        let ex = &self.ast.exprs;
        match n.kind() {
            ExprKind::ParenExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::ConstantExpr
            | ExprKind::CXXBindTemporaryExpr => self.retained_by_value(ex.operand(e), w, depth + 1),
            k if k.is_cast() => match ex.cast_kind(e) {
                CastKind::ArrayToPointerDecay => {
                    w.path.push(Step::AddressOf);
                    self.retained_by_ref(ex.operand(e), w, depth + 1);
                    w.path.pop();
                }
                CastKind::LValueToRValue => {
                    // A `const` variable's value is its initializer's.
                    let mut s = ex.operand(e);
                    // (Through the implicit move of a returned name.)
                    loop {
                        let k = ex.get(s).kind();
                        if k == ExprKind::ParenExpr
                            || (k.is_cast() && ex.cast_kind(s) == CastKind::NoOp)
                        {
                            s = ex.operand(s);
                        } else {
                            break;
                        }
                    }
                    if ex.get(s).kind() != ExprKind::DeclRefExpr {
                        return;
                    }
                    let v = ex.decl(s);
                    let Some(init) = self.ast.decls.var(v).and_then(|x| x.init) else { return };
                    let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
                    if self.kind_of(v) == DeclKind::Var
                        && self.ast.types.canonical(ty).quals().is_const()
                        && !w.on_path(v)
                    {
                        w.path.push(Step::VarInit { var: v, at: self.begin_loc(s) });
                        self.retained_by_value(init, w, depth + 1);
                        w.path.pop();
                    }
                }
                CastKind::NoOp
                | CastKind::BitCast
                | CastKind::DerivedToBase
                | CastKind::UncheckedDerivedToBase
                | CastKind::BaseToDerived
                | CastKind::Dynamic => self.retained_by_value(ex.operand(e), w, depth + 1),
                _ => {}
            },
            ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::AddrOf => {
                w.path.push(Step::AddressOf);
                self.retained_by_ref(ex.operand(e), w, depth + 1);
                w.path.pop();
            }
            ExprKind::BinaryOperator => {
                let [l, r] = ex.operands(e);
                match ex.binary_op(e) {
                    BinaryOp::Comma => self.retained_by_value(r, w, depth + 1),
                    BinaryOp::Add | BinaryOp::Sub if self.ast.types.is_pointer(n.ty()) => {
                        if self.ast.types.is_pointer(ex.get(l).ty()) {
                            self.retained_by_value(l, w, depth + 1);
                        } else if self.ast.types.is_pointer(ex.get(r).ty()) {
                            self.retained_by_value(r, w, depth + 1);
                        }
                    }
                    _ => {}
                }
            }
            ExprKind::ConditionalOperator => {
                let [t, f] = ex.cond_arms(e);
                self.retained_by_value(t, w, depth + 1);
                self.retained_by_value(f, w, depth + 1);
            }
            ExprKind::LambdaExpr => self.retained_by_closure(e, w, depth),
            _ => {}
        }
    }

    /// The locals captured by the closure `e` (clang walks each capture's
    /// initializer).
    fn retained_by_closure(&self, e: ExprId, w: &mut Walk, depth: u32) {
        let data = self.ast.exprs.lambda(e);
        let Some(inits) = data.inits else { return };
        let inits: Vec<ExprId> = self.ast.lists.iter::<ExprId>(inits).collect();
        for (cap, &init) in data.captures.iter().zip(&inits) {
            if cap.var.is_none() {
                continue;
            }
            // clang diagnoses at the capture's initializer: the name of an
            // explicit capture, the capture-default of an implicit one.
            let at = if cap.init {
                self.begin_loc(init)
            } else if cap.implicit {
                data.default_loc
            } else {
                cap.loc
            };
            w.path.push(Step::Capture { cap: *cap, at });
            if self.ast.exprs.get(init).value_category().is_glvalue() {
                self.retained_by_ref(init, w, depth + 1);
            } else {
                self.retained_by_value(init, w, depth + 1);
            }
            w.path.pop();
        }
    }

    /// The locals the glvalue `e` bound by a reference denotes (clang's
    /// `visitLocalsRetainedByReferenceBinding`).
    fn retained_by_ref(&self, e: ExprId, w: &mut Walk, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        let n = *self.ast.exprs.get(e);
        let ex = &self.ast.exprs;
        match n.kind() {
            ExprKind::ParenExpr | ExprKind::ExprWithCleanups | ExprKind::ConstantExpr => {
                self.retained_by_ref(ex.operand(e), w, depth + 1);
            }
            k if k.is_cast()
                && matches!(
                    ex.cast_kind(e),
                    CastKind::NoOp
                        | CastKind::DerivedToBase
                        | CastKind::UncheckedDerivedToBase
                        | CastKind::LValueBitCast
                ) =>
            {
                self.retained_by_ref(ex.operand(e), w, depth + 1);
            }
            ExprKind::MemberExpr if !n.has(ExprFlags::ARROW) => {
                if self.kind_of(ex.decl(e)) == DeclKind::Field {
                    self.retained_by_ref(ex.operand(e), w, depth + 1);
                }
            }
            ExprKind::ArraySubscriptExpr => {
                let [mut b, _] = ex.operands(e);
                while ex.get(b).kind() == ExprKind::ParenExpr {
                    b = ex.operand(b);
                }
                if ex.get(b).kind() == ExprKind::ImplicitCastExpr
                    && ex.cast_kind(b) == CastKind::ArrayToPointerDecay
                {
                    self.retained_by_ref(ex.operand(b), w, depth + 1);
                }
            }
            ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::Deref => {
                self.retained_by_value(ex.operand(e), w, depth + 1);
            }
            ExprKind::BinaryOperator if ex.binary_op(e) == BinaryOp::Comma => {
                self.retained_by_ref(ex.operands(e)[1], w, depth + 1);
            }
            ExprKind::ConditionalOperator => {
                let [t, f] = ex.cond_arms(e);
                self.retained_by_ref(t, w, depth + 1);
                self.retained_by_ref(f, w, depth + 1);
            }
            ExprKind::DeclRefExpr if !n.has(ExprFlags::REFERS_TO_ENCLOSING) => {
                let d = ex.decl(e);
                if !self.is_automatic_local(d) {
                    return;
                }
                let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                if !self.ast.types.is_reference(ty) {
                    w.record(Local::Var(e));
                } else if self.kind_of(d) == DeclKind::Var
                    && let Some(init) = self.ast.decls.var(d).and_then(|x| x.init)
                    && !w.on_path(d)
                {
                    // The lifetime of a reference parameter is unknown;
                    // a local reference denotes what it was bound to.
                    w.path.push(Step::VarInit { var: d, at: self.begin_loc(e) });
                    self.retained_by_ref(init, w, depth + 1);
                    w.path.pop();
                }
            }
            ExprKind::MaterializeTemporaryExpr => w.record(Local::Temp(e)),
            _ => {}
        }
    }
}
