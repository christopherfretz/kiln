//! Jump-scope checking ([stmt.dcl]/3, [except.handle]/2, [stmt.if]/2;
//! spec/sema.md §7.4): clang's `JumpScopeChecker` (`JumpDiagnostics.cpp`).
//!
//! A first walk over the body builds a tree of *protected scopes*: one
//! per variable whose initialization a jump would bypass (or whose
//! destructor an `asm goto` would skip), per try block and handler, per
//! arm of a constexpr / consteval `if`, per GNU statement expression and
//! per lifetime-extended temporary with a destructor. Every label, case
//! and jump records the scope it is in. A `goto` or a `switch` may leave
//! scopes (codegen runs their cleanups) but not enter one with an
//! "in" diagnostic; an `asm goto` may do neither, because `callbr` cannot
//! run cleanups. A GNU indirect `goto *p` may reach every label whose
//! address is taken (`&&l`; not an `asm goto` label), so it is
//! checked against each scope holding such a label with the `asm goto`
//! rule (clang `VerifyIndirectJumps`); `goto *&&l` is an ordinary goto.
//!
//! The walk enters the bodies of the lambdas the function contains (as
//! clang's does, WP-455): their labels and jumps join the function's, so
//! an outer `goto *p` is checked against a label whose address a lambda
//! takes.

use kiln_ast::decl::{DeclKind, InitStyle, VarStorage};
use kiln_ast::expr::{CastKind, ExprKind, StorageDuration};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::Sema;

/// One protected scope (clang `GotoScope`).
#[derive(Clone, Copy, Debug)]
struct GotoScope {
    /// The enclosing scope (`u32::MAX` for the function's own).
    parent: u32,
    /// The note for a jump into this scope (`None`: entering is fine).
    in_diag: Option<DiagId>,
    /// The note for an `asm goto` out of this scope.
    out_diag: Option<DiagId>,
    loc: Loc,
}

struct Checker<'s, 'a, 'sm> {
    sema: &'s Sema<'a, 'sm>,
    scopes: Vec<GotoScope>,
    /// The scope of every label, case, default, goto, switch and
    /// `asm goto` (clang `LabelAndGotoScopes`).
    at: FxHashMap<StmtId, u32>,
    /// The `LabelStmt` of each label declaration.
    label_stmts: FxHashMap<DeclId, StmtId>,
    /// gotos, switches and `asm goto`s, in walk order.
    jumps: Vec<StmtId>,
    /// Indirect gotos without a constant target (`IndirectJumps`).
    indirect_jumps: Vec<StmtId>,
    /// Labels whose address is taken, in walk order
    /// (`IndirectJumpTargets`).
    indirect_targets: Vec<DeclId>,
    /// `[[clang::musttail]]` return statements (`MustTailStmts`).
    musttail: Vec<StmtId>,
}

impl Sema<'_, '_> {
    /// clang `DiagnoseInvalidJumps`: checks every goto, `asm goto` and
    /// switch of the function body `body`.
    pub(crate) fn check_jump_scopes(&self, body: StmtId) {
        let mut c = Checker {
            sema: self,
            scopes: vec![GotoScope {
                parent: u32::MAX,
                in_diag: None,
                out_diag: None,
                loc: Loc::INVALID,
            }],
            at: FxHashMap::default(),
            label_stmts: FxHashMap::default(),
            jumps: Vec::new(),
            indirect_jumps: Vec::new(),
            indirect_targets: Vec::new(),
            musttail: Vec::new(),
        };
        let mut parent = 0;
        c.stmt(body, &mut parent);
        c.verify_jumps();
        c.verify_indirect_jumps();
        c.verify_musttail();
    }
}

impl Checker<'_, '_, '_> {
    fn push(
        &mut self,
        parent: u32,
        in_diag: Option<DiagId>,
        out_diag: Option<DiagId>,
        loc: Loc,
    ) -> u32 {
        self.scopes.push(GotoScope { parent, in_diag, out_diag, loc });
        u32::try_from(self.scopes.len() - 1).expect("scope count fits u32")
    }

    /// `BuildScopeInformation(Stmt*)` for a statement: scopes opened
    /// inside it do not outlive it, except a declaration statement's,
    /// which extend to the end of the enclosing block (`parent` is the
    /// enclosing block's running scope).
    fn stmt(&mut self, s: StmtId, parent: &mut u32) {
        let ast = &self.sema.ast;
        let st = &ast.stmts;
        let n = *st.get(s);
        if n.kind() == StmtKind::DeclStmt {
            for d in ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)).collect::<Vec<_>>() {
                self.decl(d, parent);
            }
            return;
        }
        // Cases, defaults and labels are not scope parents: a declaration
        // labelled by one extends to the end of the enclosing block.
        match n.kind() {
            StmtKind::CaseStmt => {
                self.at.insert(s, *parent);
                let sub = st.case_data(s).sub;
                return self.stmt(sub, parent);
            }
            StmtKind::DefaultStmt => {
                self.at.insert(s, *parent);
                return self.stmt(st.default_sub(s), parent);
            }
            StmtKind::LabelStmt => {
                self.at.insert(s, *parent);
                self.label_stmts.insert(DeclId::new(n.a()), s);
                return self.stmt(StmtId::new(n.b()), parent);
            }
            _ => {}
        }
        let mut local = *parent;
        let p = &mut local;
        match n.kind() {
            StmtKind::CompoundStmt => {
                for c in ast.lists.iter::<StmtId>(st.compound_body(s)).collect::<Vec<_>>() {
                    self.stmt(c, p);
                }
            }
            StmtKind::ExprStmt => self.expr(st.expr_stmt_expr(s), parent),
            StmtKind::IfStmt => self.if_stmt(s, p),
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                self.opt_stmt(d.init, p);
                if let Some(v) = d.cond_var {
                    self.decl(v, p);
                }
                self.record_jump(s, *p);
                self.expr(d.cond, p);
                self.stmt(d.body, p);
            }
            StmtKind::AttributedStmt => {
                let musttail = ast
                    .lists
                    .iter::<kiln_ast::AttrId>(st.stmt_attrs(s))
                    .any(|a| ast.attrs.get(a).kind == kiln_ast::attr::AttrKind::MUSTTAIL);
                if musttail {
                    self.at.insert(s, *p);
                    self.musttail.push(s);
                }
                self.stmt(StmtId::new(n.b()), p);
            }
            StmtKind::GotoStmt => self.record_jump(s, *p),
            StmtKind::IndirectGotoStmt => {
                // `goto *&&l` is an ordinary goto, and its operand is not
                // walked (it takes no address for the other jumps).
                if ast.indirect_goto_constant_target(s).is_some() {
                    return self.record_jump(s, *p);
                }
                self.at.insert(s, *p);
                self.indirect_jumps.push(s);
                self.expr(st.indirect_goto_target(s), p);
            }
            StmtKind::GCCAsmStmt => {
                if st.asm_bits(s) & kiln_ast::stmt::asm_bits::GOTO != 0 {
                    self.record_jump(s, *p);
                }
                for op in st.asm_operands(s).to_vec() {
                    self.expr(op.expr, p);
                }
            }
            StmtKind::WhileStmt
            | StmtKind::ForStmt
            | StmtKind::DoStmt
            | StmtKind::CXXForRangeStmt => self.loop_stmt(s, p),
            StmtKind::CXXExpansionStmt => {
                for c in self.sema.children(s) {
                    self.stmt(c, p);
                }
            }
            StmtKind::ReturnStmt => self.opt_expr(st.return_value(s), p),
            StmtKind::CoreturnStmt => self.opt_expr(st.coreturn_operand(s), p),
            StmtKind::CXXTryStmt => self.try_stmt(s, p),
            _ => {}
        }
    }

    /// A loop statement: its init, condition variable, condition,
    /// increment and body all live in the loop's own scope.
    fn loop_stmt(&mut self, s: StmtId, p: &mut u32) {
        let st = &self.sema.ast.stmts;
        match st.get(s).kind() {
            StmtKind::WhileStmt | StmtKind::ForStmt => {
                let d = *st.loop_data(s);
                self.opt_stmt(d.init, p);
                if let Some(v) = d.cond_var {
                    self.decl(v, p);
                }
                self.opt_expr(d.cond, p);
                self.opt_expr(d.inc, p);
                self.opt_stmt(d.body, p);
            }
            StmtKind::DoStmt => {
                let d = *st.loop_data(s);
                self.opt_stmt(d.body, p);
                self.opt_expr(d.cond, p);
            }
            StmtKind::CXXForRangeStmt => {
                let d = *st.range_for_data(s);
                self.opt_stmt(d.init, p);
                self.decl(d.range, p);
                if let Some(b) = d.begin {
                    self.decl(b, p);
                }
                if let Some(e) = d.end {
                    self.decl(e, p);
                }
                self.opt_expr(d.cond, p);
                self.opt_expr(d.inc, p);
                self.decl(d.loop_var, p);
                self.stmt(d.body, p);
            }
            _ => unreachable!("loop_stmt on a non-loop statement"),
        }
    }

    /// An `if`: the arms of a constexpr / consteval `if` are protected
    /// ([stmt.if]/2), as is a constexpr `if`'s condition.
    fn if_stmt(&mut self, s: StmtId, p: &mut u32) {
        let st = &self.sema.ast.stmts;
        let n = *st.get(s);
        let d = *st.if_data(s);
        let kind = st.if_kind(s);
        if kind == IfKind::Ordinary {
            self.opt_stmt(d.init, p);
            if let Some(v) = d.cond_var {
                self.decl(v, p);
            }
            self.opt_expr(d.cond, p);
            self.opt_stmt(d.then, p);
            self.opt_stmt(d.els, p);
            return;
        }
        let diag = if kind == IfKind::Constexpr {
            DiagId::note_protected_by_constexpr_if
        } else {
            DiagId::note_protected_by_consteval_if
        };
        if let Some(v) = d.cond_var {
            self.decl(v, p);
        }
        // Neither the condition nor either arm may be entered.
        let mut np = self.push(*p, Some(diag), None, n.loc());
        if kind == IfKind::Constexpr {
            self.opt_expr(d.cond, &mut np);
        }
        let mut np = self.push(*p, Some(diag), None, n.loc());
        self.opt_stmt(d.then, &mut np);
        if let Some(e) = d.els {
            let mut np = self.push(*p, Some(diag), None, n.loc());
            self.stmt(e, &mut np);
        }
    }

    /// A try block and each handler are protected scopes
    /// ([except.handle]/2).
    fn try_stmt(&mut self, s: StmtId, p: &mut u32) {
        let ast = &self.sema.ast;
        let st = &ast.stmts;
        let loc = st.get(s).loc();
        let mut np = self.push(
            *p,
            Some(DiagId::note_protected_by_cxx_try),
            Some(DiagId::note_exits_cxx_try),
            loc,
        );
        self.stmt(st.try_block(s), &mut np);
        // A jump from a handler into the try block (or another
        // handler) is not allowed either.
        for h in ast.lists.iter::<StmtId>(st.try_handlers(s)).collect::<Vec<_>>() {
            let mut np = self.push(
                *p,
                Some(DiagId::note_protected_by_cxx_catch),
                Some(DiagId::note_exits_cxx_catch),
                st.get(h).loc(),
            );
            self.stmt(st.catch_block(h), &mut np);
        }
    }

    fn opt_stmt(&mut self, s: Option<StmtId>, parent: &mut u32) {
        if let Some(s) = s {
            self.stmt(s, parent);
        }
    }

    fn opt_expr(&mut self, e: Option<ExprId>, parent: &mut u32) {
        if let Some(e) = e {
            self.expr(e, parent);
        }
    }

    fn record_jump(&mut self, s: StmtId, scope: u32) {
        self.at.insert(s, scope);
        self.jumps.push(s);
    }

    /// `BuildScopeInformation(Stmt*)` for an expression: a scope opened
    /// inside it (a lifetime-extended temporary) lasts as long as the
    /// enclosing statement's.
    fn expr(&mut self, e: ExprId, parent: &mut u32) {
        let ast = &self.sema.ast;
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::StmtExpr => {
                // [GNU] Jumping into a statement expression is not
                // permitted; jumping out of one is.
                let mut np = self.push(
                    *parent,
                    Some(DiagId::note_enters_statement_expression),
                    None,
                    n.loc(),
                );
                self.stmt(ast.exprs.stmt_expr_body(e), &mut np);
                return;
            }
            ExprKind::MaterializeTemporaryExpr
                if ast.exprs.temporary_storage(e) == StorageDuration::Automatic =>
            {
                if let Some(&sub) = ast.exprs.children(e, &ast.lists).first() {
                    let obj = self.skip_rvalue_adjustments(sub);
                    if ast.needs_destruction(ast.exprs.get(obj).ty()) {
                        *parent = self.push(
                            *parent,
                            None,
                            Some(DiagId::note_exits_temporary_dtor),
                            ast.exprs.get(obj).loc(),
                        );
                    }
                }
            }
            ExprKind::AddrLabelExpr => self.indirect_targets.push(ast.exprs.addr_label(e)),
            // The copy of a default argument rebuilt for a lifetime-extending
            // use (spec/sema.md §7.6a): its extended temporaries live as long
            // as the enclosing statement's scope, so an indirect goto out of
            // it skips their destructors. clang 19/22 do not look (their
            // `CXXDefaultArgExpr` has no children). WP-1166.
            ExprKind::CXXDefaultArgExpr => {
                if let Some(r) = ast.exprs.default_arg_rewritten(e) {
                    self.expr(r, parent);
                }
                return;
            }
            _ => {}
        }
        for c in ast.exprs.children(e, &ast.lists) {
            self.expr(c, parent);
        }
        // clang's `LambdaExpr::children` ends with the body: its labels,
        // address-taken labels and jumps join the enclosing function's
        // (an outer `goto *p` may reach a label whose address the lambda
        // takes; a bad jump inside it is diagnosed by both functions).
        // A local class's members and default arguments are not walked.
        if n.kind() == ExprKind::LambdaExpr
            && let Some(body) = ast.exprs.lambdas.get(kiln_ast::LambdaId::new(n.a())).body
        {
            let mut p = *parent;
            self.stmt(body, &mut p);
        }
    }

    /// clang `Expr::skipRValueSubobjectAdjustments` (derived-to-base and
    /// no-op casts, member accesses).
    fn skip_rvalue_adjustments(&self, mut e: ExprId) -> ExprId {
        let ex = &self.sema.ast.exprs;
        for _ in 0..1024 {
            let k = ex.get(e).kind();
            let peel = k == ExprKind::MemberExpr
                || (k.is_cast()
                    && matches!(
                        ex.cast_kind(e),
                        CastKind::NoOp | CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase
                    ));
            let next =
                if peel { ex.children(e, &self.sema.ast.lists).first().copied() } else { None };
            match next {
                Some(x) => e = x,
                None => break,
            }
        }
        e
    }

    /// `BuildScopeInformation(Decl*)`: a declaration that protects its
    /// scope opens one, and its initializer is walked inside it.
    fn decl(&mut self, d: DeclId, parent: &mut u32) {
        let (in_diag, out_diag) = self.diags_for_decl(d);
        if in_diag.is_some() || out_diag.is_some() {
            let loc = self.sema.ast.decls.get(d).loc();
            *parent = self.push(*parent, in_diag, out_diag, loc);
        }
        if let Some(init) = self.sema.ast.decls.var(d).and_then(|v| v.init) {
            self.expr(init, parent);
        }
    }

    /// clang `GetDiagForGotoScopeDecl`.
    fn diags_for_decl(&self, d: DeclId) -> (Option<DiagId>, Option<DiagId>) {
        let sema = self.sema;
        let ast = &sema.ast;
        let n = ast.decls.get(d);
        match n.kind() {
            DeclKind::Var => {}
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let vm = n.ty().is_some_and(|t| self.variably_modified(t));
                let id = if n.kind() == DeclKind::Typedef {
                    DiagId::note_protected_by_vla_typedef
                } else {
                    DiagId::note_protected_by_vla_type_alias
                };
                return (vm.then_some(id), None);
            }
            _ => return (None, None),
        }
        let Some(v) = ast.decls.var(d) else { return (None, None) };
        let Some(ty) = n.ty() else { return (None, None) };
        if ast
            .lists
            .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(d))
            .any(|a| ast.attrs.get(a).kind == kiln_ast::attr::AttrKind::CLEANUP)
        {
            return (Some(DiagId::note_protected_by_cleanup), Some(DiagId::note_exits_cleanup));
        }
        let mut in_diag = self.variably_modified(ty).then_some(DiagId::note_protected_by_vla);
        let local = !matches!(v.spec.storage(), VarStorage::Static | VarStorage::Extern)
            && v.spec.tls() == kiln_ast::decl::TlsKind::None;
        let out_diag = (local && ast.needs_destruction(ty)).then_some(DiagId::note_exits_dtor);
        if let Some(init) = v.init
            && local
            && !self.contains_errors(init)
        {
            // [stmt.dcl]/3: bypassing a declaration with an initializer
            // is ill-formed, unless it is default-initialization by a
            // trivial default constructor of a class with a trivial
            // destructor (a non-POD such class is only a C++98 error).
            in_diag = Some(DiagId::note_protected_by_variable_init);
            if ast.exprs.get(init).kind() == ExprKind::CXXConstructExpr
                && v.spec.init_style() == InitStyle::Call
                && ast.lists.is_empty(ast.exprs.construct_args(init))
                && ast
                    .decls
                    .function(ast.exprs.constructor(init))
                    .is_some_and(|f| f.spec.is_trivial())
            {
                in_diag = out_diag.map(|_| DiagId::note_protected_by_variable_nontriv_destructor);
            }
        }
        (in_diag, out_diag)
    }

    /// `Expr::containsErrors`, approximated: a recovery expression or an
    /// error type at the top.
    fn contains_errors(&self, e: ExprId) -> bool {
        let x = self.sema.ast.exprs.get(e);
        x.kind() == ExprKind::RecoveryExpr || self.sema.is_error_type(x.ty())
    }

    /// `Type::isVariablyModifiedType`.
    fn variably_modified(&self, q: QualType) -> bool {
        let t = &self.sema.ast.types;
        let mut q = q;
        for _ in 0..256 {
            q = match t.canonical_view(q) {
                TypeView::VariableArray { .. } => return true,
                TypeView::ConstantArray(a) => a.elem,
                TypeView::IncompleteArray(e)
                | TypeView::Pointer(e)
                | TypeView::LValueRef(e)
                | TypeView::RValueRef(e) => e,
                _ => return false,
            };
        }
        false
    }

    /// clang `GetDeepestCommonScope`: inner scopes have higher indices.
    fn common_scope(&self, mut a: u32, mut b: u32) -> u32 {
        while a != b {
            if a < b {
                b = self.scopes[b as usize].parent;
            } else {
                a = self.scopes[a as usize].parent;
            }
        }
        a
    }

    /// clang `VerifyJumps` (the jumps are checked last first).
    fn verify_jumps(&mut self) {
        let st = &self.sema.ast.stmts;
        while let Some(j) = self.jumps.pop() {
            let n = *st.get(j);
            match n.kind() {
                StmtKind::GotoStmt => {
                    if let Some(&to) = self.label_stmts.get(&DeclId::new(n.a())) {
                        self.check_jump(j, to, n.loc(), DiagId::sema_goto_into_protected_scope);
                    }
                }
                StmtKind::IndirectGotoStmt => {
                    let target = self.sema.ast.indirect_goto_constant_target(j);
                    if let Some(&to) = target.and_then(|l| self.label_stmts.get(&l)) {
                        self.check_jump(j, to, n.loc(), DiagId::sema_goto_into_protected_scope);
                    }
                }
                StmtKind::GCCAsmStmt => {
                    let labels: Vec<DeclId> =
                        self.sema.ast.lists.iter(st.asm_data(j).labels).collect();
                    for l in labels {
                        let Some(&to) = self.label_stmts.get(&l) else { continue };
                        let (from_scope, to_scope) = (self.at[&j], self.at[&to]);
                        if from_scope != to_scope {
                            self.diagnose_indirect_jump(j, from_scope, to, to_scope);
                        }
                    }
                }
                StmtKind::SwitchStmt => {
                    // clang's `getSwitchCaseList` is in reverse order.
                    let cases: Vec<StmtId> =
                        self.sema.ast.lists.iter(st.switch_data(j).cases).collect();
                    for c in cases.into_iter().rev() {
                        if self.at.contains_key(&c) {
                            let loc = st.get(c).loc();
                            self.check_jump(j, c, loc, DiagId::sema_switch_into_protected_scope);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// clang `CheckJump`: leaving scopes is fine; entering one with an
    /// "in" note is an error listing them innermost first.
    fn check_jump(&self, from: StmtId, to: StmtId, loc: Loc, err: DiagId) {
        let (Some(&fs), Some(&ts)) = (self.at.get(&from), self.at.get(&to)) else { return };
        if fs == ts {
            return;
        }
        let common = self.common_scope(fs, ts);
        if common == ts {
            return;
        }
        let mut msg = None;
        let mut i = ts;
        while i != common {
            let sc = self.scopes[i as usize];
            if let Some(id) = sc.in_diag {
                let d = msg.take().unwrap_or_else(|| self.sema.diag(err, loc));
                msg = Some(d.with_note(self.sema.diag(id, sc.loc)));
            }
            i = sc.parent;
        }
        if let Some(d) = msg {
            self.sema.emit(d);
        }
    }

    /// clang `VerifyMustTailStmts`: a `musttail` return may not leave a
    /// scope with an "out" note (a destructor or a try block would have
    /// to run after the call), one error per such scope (WP-809).
    fn verify_musttail(&self) {
        for &s in &self.musttail {
            let loc = self.sema.ast.stmts.get(s).loc();
            let mut i = self.at[&s];
            while i != 0 && i != u32::MAX {
                let sc = self.scopes[i as usize];
                if let Some(id) = sc.out_diag {
                    let msg = self
                        .sema
                        .diag(DiagId::sema_musttail_scope, loc)
                        .with_note(self.sema.diag(id, sc.loc));
                    self.sema.emit(msg);
                }
                i = sc.parent;
            }
        }
    }

    /// clang `VerifyIndirectJumps`: with no label address taken, the
    /// first indirect goto is an error; otherwise every scope holding an
    /// indirect goto must reach every scope holding an address-taken
    /// label by leaving only scopes without an "out" note, then entering
    /// only scopes without an "in" note. One jump and one label stand for
    /// each scope, visited in clang's `DenseMap` order.
    fn verify_indirect_jumps(&self) {
        let Some(&first) = self.indirect_jumps.first() else { return };
        if self.indirect_targets.is_empty() {
            let loc = self.sema.ast.stmts.get(first).loc();
            self.sema.emit(self.sema.diag(DiagId::sema_indirect_goto_without_addrlabel, loc));
            return;
        }
        let jump_scopes = dense_map_order(
            self.indirect_jumps.iter().filter_map(|&j| Some((*self.at.get(&j)?, j))),
        );
        let target_scopes = dense_map_order(self.indirect_targets.iter().filter_map(|l| {
            let s = *self.label_stmts.get(l)?;
            Some((*self.at.get(&s)?, s))
        }));
        let mut reachable = vec![false; self.scopes.len()];
        for &(target_scope, target) in &target_scopes {
            reachable.fill(false);
            // The scopes from which the target's can be entered; `min` is
            // the shallowest.
            let mut min = target_scope;
            loop {
                reachable[min as usize] = true;
                if min == 0 || self.scopes[min as usize].in_diag.is_some() {
                    break;
                }
                min = self.scopes[min as usize].parent;
            }
            for &(jump_scope, jump) in &jump_scopes {
                let mut scope = jump_scope;
                let ok = loop {
                    if reachable[scope as usize] {
                        let mut s = jump_scope;
                        while s != scope {
                            reachable[s as usize] = true;
                            s = self.scopes[s as usize].parent;
                        }
                        break true;
                    }
                    if scope == 0 || scope < min || self.scopes[scope as usize].out_diag.is_some() {
                        break false;
                    }
                    scope = self.scopes[scope as usize].parent;
                };
                if !ok {
                    self.diagnose_indirect_jump(jump, jump_scope, target, target_scope);
                }
            }
        }
    }

    /// clang `DiagnoseIndirectOrAsmJump` for an indirect or `asm goto`
    /// `jump` to the label statement `target`: neither leaving a scope
    /// with an "out" note nor entering one with an "in" note is allowed.
    fn diagnose_indirect_jump(&self, jump: StmtId, from: u32, target: StmtId, to: u32) {
        let st = &self.sema.ast.stmts;
        if from == to {
            return;
        }
        let asm = u8::from(st.get(jump).kind() == StmtKind::GCCAsmStmt);
        let common = self.common_scope(from, to);
        let mut msg: Option<kiln_diag::Diag> = None;
        let note = |msg: &mut Option<kiln_diag::Diag>, id: DiagId, loc: Loc| {
            let d = msg.take().unwrap_or_else(|| {
                self.sema
                    .diag(DiagId::sema_indirect_goto_in_protected_scope, st.get(jump).loc())
                    // One per target scope at the same jump, as clang.
                    .allow_duplicate()
                    .arg(asm)
                    .with_note(
                        self.sema
                            .diag(DiagId::note_indirect_goto_target, st.get(target).loc())
                            .arg(asm),
                    )
            });
            *msg = Some(d.with_note(self.sema.diag(id, loc)));
        };
        let mut i = from;
        while i != common {
            let sc = self.scopes[i as usize];
            if let Some(id) = sc.out_diag {
                note(&mut msg, id, sc.loc);
            }
            i = sc.parent;
        }
        let mut i = to;
        while i != common {
            let sc = self.scopes[i as usize];
            if let Some(id) = sc.in_diag {
                note(&mut msg, id, sc.loc);
            }
            i = sc.parent;
        }
        if let Some(d) = msg {
            self.sema.emit(d);
        }
    }
}

/// The distinct keys of `items` with the first value for each, in the
/// iteration order of an LLVM `DenseMap<unsigned, T>` built by inserting
/// them in turn (hash `k * 37`, quadratic probing, 64 buckets growing
/// ×2 at three-quarters full): clang visits jump and target scopes in it.
fn dense_map_order<T: Copy>(items: impl Iterator<Item = (u32, T)>) -> Vec<(u32, T)> {
    fn slot<T>(table: &[Option<(u32, T)>], k: u32) -> (usize, bool) {
        let mask = table.len() - 1;
        let mut b = (k.wrapping_mul(37) as usize) & mask;
        let mut probe = 1;
        loop {
            match &table[b] {
                None => return (b, false),
                Some((x, _)) if *x == k => return (b, true),
                Some(_) => {
                    b = (b + probe) & mask;
                    probe += 1;
                }
            }
        }
    }
    let mut table: Vec<Option<(u32, T)>> = Vec::new();
    let mut n = 0usize;
    for (k, v) in items {
        if table.is_empty() {
            table.resize_with(64, || None);
        }
        if slot(&table, k).1 {
            continue;
        }
        if (n + 1) * 4 >= table.len() * 3 {
            let old = std::mem::take(&mut table);
            table.resize_with(old.len() * 2, || None);
            for (ok, ov) in old.into_iter().flatten() {
                let (b, _) = slot(&table, ok);
                table[b] = Some((ok, ov));
            }
        }
        let (b, _) = slot(&table, k);
        table[b] = Some((k, v));
        n += 1;
    }
    table.into_iter().flatten().collect()
}
