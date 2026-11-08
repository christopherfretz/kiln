//! Statement-chunk outlining of oversized compound statements (WP-641,
//! spec/landed/codegen/WP-641.md).
//!
//! Some LLVM function passes (jump threading's block merging, the
//! memset merging of `MemCpyOptPass`) are super-linear in the size of ONE
//! function. A compound statement holding thousands of expression
//! statements (a generated registration function: `t.def(...);
//! t.def(...); ...`) makes one function that `opt -O2` takes minutes on,
//! although every statement is independent. Above a size threshold kiln emits runs of such statements
//! into internal `noinline` helpers, each called from the original
//! function with the addresses of the locals it names (and `this` and the
//! closure object): the giant function becomes a few hundred calls, every
//! helper is small, and each pass is linear in their total.
//!
//! The transformation is invisible: the helpers have internal linkage, an
//! exception thrown in one unwinds out of it after its own temporaries'
//! cleanups to the caller's landing pad (the same cleanup order as the
//! inline emission), and a compound below the threshold is emitted
//! exactly as before.

use kiln_ast::decl::DeclKind;
use kiln_ast::decl::builtin as id;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, LambdaId, StmtId};
use kiln_llvm::{Attr, AttrKind, CallOpts, FuncId, Linkage, TypeId as LTy, Value};
use kiln_source::fx::FxHashSet;

use crate::cleanup::JumpDest;
use crate::fir::{Fir, Op, V};
use crate::func::{Addr, Fx, Jump, RetKind};
use crate::module::{Cg, DefFnAttrs, DefInline, R, Sig};

/// A compound statement is outlined when it directly holds at least this
/// many outlinable expression statements (measured,
/// spec/landed/codegen/WP-641.md: below it no function is slow enough to
/// matter).
pub(crate) const MIN_STMTS: usize = 256;
/// Each helper holds at most this many statements.
pub(crate) const CHUNK: usize = 32;
/// A run of consecutive outlinable statements shorter than this stays
/// inline (a call per one or two statements gains nothing).
pub(crate) const MIN_RUN: usize = 4;

/// An expansion element smaller than this (statements and expressions)
/// stays inline even in an outlined expansion statement: its call would
/// cost more than its body.
pub(crate) const MIN_ELEMENT: usize = 16;

/// How the helper of an expansion element returns to its caller
/// ([`Cg::emit_outlined_element`]): falling off its end or `continue`,
/// a `break` of the expansion statement, or a `return` of the function.
const ELEM_NEXT: i128 = 0;
const ELEM_BREAK: i128 = 1;
const ELEM_RETURN: i128 = 2;

/// What [`Cg::scan_element`] learnt of an expansion element.
pub(crate) struct ElemScan {
    /// A helper can emit it.
    ok: bool,
    /// Its statements and expressions.
    size: usize,
    /// It breaks out of the expansion statement.
    brk: bool,
    /// It returns from the function.
    ret: bool,
    /// Its full-expressions (declaration initializers included).
    exprs: Vec<ExprId>,
}

/// What a helper receives: the addresses of the caller's locals it names,
/// then `this`, then the closure object.
struct Captures {
    locals: Vec<(DeclId, Addr)>,
    this: Option<V>,
    closure: Option<(V, LambdaId)>,
}

impl Cg<'_> {
    /// Emits the statements `list` of a compound statement, outlining
    /// runs of them into helpers when the compound is large enough.
    pub(crate) fn emit_stmt_list(&mut self, list: &[StmtId]) -> R<()> {
        let Some(chunks) = self.outline_plan(list) else {
            for &s in list {
                self.emit_stmt(s)?;
            }
            return Ok(());
        };
        let mut i = 0;
        for (start, end) in chunks {
            for &s in &list[i..start] {
                self.emit_stmt(s)?;
            }
            self.emit_outlined(&list[start..end])?;
            i = end;
        }
        for &s in &list[i..] {
            self.emit_stmt(s)?;
        }
        Ok(())
    }

    /// The `[start, end)` ranges of `list` to outline, or `None` to emit
    /// the compound inline.
    fn outline_plan(&self, list: &[StmtId]) -> Option<Vec<(usize, usize)>> {
        if list.len() < MIN_STMTS || !self.outline_enabled() {
            return None;
        }
        let ok: Vec<bool> = list.iter().map(|&s| self.stmt_outlinable(s)).collect();
        if ok.iter().filter(|&&b| b).count() < MIN_STMTS {
            return None;
        }
        let mut chunks = Vec::new();
        let mut i = 0;
        while i < ok.len() {
            if !ok[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < ok.len() && ok[i] {
                i += 1;
            }
            let len = i - start;
            if len < MIN_RUN {
                continue;
            }
            // Balanced chunks of at most CHUNK statements.
            let n = len.div_ceil(CHUNK);
            for k in 0..n {
                chunks.push((start + k * len / n, start + (k + 1) * len / n));
            }
        }
        (!chunks.is_empty()).then_some(chunks)
    }

    /// The function being emitted may have its statements outlined: it is
    /// optimized, and none of its state a helper could not reproduce is
    /// live (a coroutine, a structor, an explicit object parameter, a VLA
    /// bound, a conditional or opaque-value context).
    fn outline_enabled(&self) -> bool {
        let f = &self.f;
        let Some(d) = f.decl else { return false };
        self.opts.optimize()
            && f.func.is_some()
            && !f.naked
            && f.coro.is_none()
            && f.await_frame.is_none()
            && f.structor.is_none()
            && f.closure_obj.is_none()
            && f.inh_args.is_none()
            && f.vla_sizes.is_empty()
            && f.conds.is_empty()
            && f.opaque.is_empty()
            && f.array_index.is_empty()
            && self.def_inline_attrs(d) != DefInline::OptNone
    }

    /// Whether `s` is an expression statement worth outlining and that a
    /// helper can evaluate: it calls or constructs something (a statement
    /// of plain arithmetic on locals stays inline, where SROA keeps the
    /// locals in registers), and has no statement expression, label
    /// address, `co_await`, splice or unexpanded construct, no call of a
    /// builtin bound to its function's frame (`alloca`, `va_start`, frame
    /// or return address), of `__builtin_object_size` (a helper sees a
    /// parameter, not the caller's object) or of a function that returns
    /// twice.
    fn stmt_outlinable(&self, s: StmtId) -> bool {
        let ast = self.ast;
        ast.stmts.get(s).kind() == StmtKind::ExprStmt
            && self.expr_outlinable(ast.stmts.expr_stmt_expr(s), &mut 0) == Some(true)
    }

    /// Whether a helper can evaluate expression `root` (`None` if not;
    /// else whether it calls or constructs something), adding its node
    /// count to `size`.
    fn expr_outlinable(&self, root: ExprId, size: &mut usize) -> Option<bool> {
        let ast = self.ast;
        let mut stack = vec![root];
        let mut kids = Vec::new();
        let mut calls = false;
        while let Some(e) = stack.pop() {
            *size += 1;
            let k = ast.exprs.get(e).kind();
            calls |= matches!(
                k,
                ExprKind::CallExpr
                    | ExprKind::CXXMemberCallExpr
                    | ExprKind::CXXOperatorCallExpr
                    | ExprKind::CXXConstructExpr
                    | ExprKind::CXXTemporaryObjectExpr
                    | ExprKind::CXXNewExpr
                    | ExprKind::CXXDeleteExpr
                    | ExprKind::CXXThrowExpr
            );
            match k {
                ExprKind::StmtExpr
                | ExprKind::AddrLabelExpr
                | ExprKind::CoawaitExpr
                | ExprKind::CoyieldExpr
                | ExprKind::DependentCoawaitExpr
                | ExprKind::CXXSpliceExpr
                | ExprKind::UserDefinedLiteral
                | ExprKind::PackIndexingExpr
                | ExprKind::CXXExpansionSelectExpr
                | ExprKind::RecoveryExpr
                | ExprKind::UnresolvedLookupExpr
                | ExprKind::UnresolvedMemberExpr
                | ExprKind::DependentScopeDeclRefExpr
                | ExprKind::CXXDependentScopeMemberExpr
                | ExprKind::SubstNonTypeTemplateParmPackExpr
                | ExprKind::FunctionParmPackExpr
                | ExprKind::PackExpansionExpr
                | ExprKind::CXXFoldExpr
                | ExprKind::CXXUnresolvedConstructExpr
                | ExprKind::ParenListExpr
                | ExprKind::CXXInheritedCtorInitExpr => return None,
                ExprKind::CallExpr if !self.call_outlinable(e) => return None,
                ExprKind::DeclRefExpr => {
                    let d = ast.exprs.decl(e);
                    if ast.decls.get(d).kind() == DeclKind::Binding
                        && let Some(b) = ast.decls.binding_expr(d)
                    {
                        stack.push(b);
                    }
                }
                _ => {}
            }
            kids.clear();
            ast.exprs.children_into(e, &ast.lists, &mut kids);
            stack.extend_from_slice(&kids);
        }
        Some(calls)
    }

    /// A call `e` that may run in a helper.
    fn call_outlinable(&self, e: ExprId) -> bool {
        let ast = self.ast;
        if let Some((_, b)) = crate::builtins::builtin_callee(ast, e)
            && matches!(
                b,
                id::ALLOCA
                    | id::ALLOCA_WITH_ALIGN
                    | id::VA_START
                    | id::FRAME_ADDRESS
                    | id::RETURN_ADDRESS
                    | id::OBJECT_SIZE
                    | id::DYNAMIC_OBJECT_SIZE
            )
        {
            return false;
        }
        let mut c = ast.exprs.callee(e);
        while matches!(ast.exprs.get(c).kind(), ExprKind::ParenExpr | ExprKind::ImplicitCastExpr) {
            c = ast.exprs.operand(c);
        }
        if ast.exprs.get(c).kind() == ExprKind::DeclRefExpr {
            let d = ast.exprs.decl(c);
            // A helper would return before the second return.
            if ast.decls.get(d).kind().is_function() && crate::builtins::returns_twice(ast, d) {
                return false;
            }
        }
        true
    }

    /// The locals of the current function the statements `stmts` name
    /// (in first-reference order), with `this` and the closure object.
    fn outline_captures(&self, stmts: &[StmtId]) -> Captures {
        let ast = self.ast;
        let roots: Vec<ExprId> = stmts.iter().map(|&s| ast.stmts.expr_stmt_expr(s)).collect();
        self.captures_of(&roots)
    }

    /// The locals of the current function the expressions `roots` name
    /// (in first-reference order, through structured bindings and the
    /// sources of opaque values), with `this` and the closure object.
    fn captures_of(&self, roots: &[ExprId]) -> Captures {
        let ast = self.ast;
        let mut seen = FxHashSet::default();
        let mut locals = Vec::new();
        let mut kids = Vec::new();
        for &r in roots {
            let mut stack = vec![r];
            while let Some(e) = stack.pop() {
                match ast.exprs.get(e).kind() {
                    ExprKind::DeclRefExpr => {
                        let d = ast.exprs.decl(e);
                        if let Some(&a) = self.f.locals.get(&d) {
                            if seen.insert(d) {
                                locals.push((d, a));
                            }
                        } else if ast.decls.get(d).kind() == DeclKind::Binding
                            && let Some(b) = ast.decls.binding_expr(d)
                        {
                            stack.push(b);
                        }
                    }
                    ExprKind::OpaqueValueExpr => stack.extend(ast.exprs.opaque_source(e)),
                    _ => {}
                }
                kids.clear();
                ast.exprs.children_into(e, &ast.lists, &mut kids);
                // Reversed: the walk visits children in source order.
                stack.extend(kids.iter().rev());
            }
        }
        Captures { locals, this: self.f.this, closure: self.f.closure }
    }

    /// Emits `stmts` into a new helper and calls it.
    fn emit_outlined(&mut self, stmts: &[StmtId]) -> R<()> {
        if self.f.fir.cur().is_none() {
            // Unreachable statements without labels emit nothing.
            return Ok(());
        }
        let cap = self.outline_captures(stmts);
        let mut args: Vec<V> = cap.locals.iter().map(|&(_, a)| a.v).collect();
        args.extend(cap.this);
        args.extend(cap.closure.map(|(v, _)| v));
        let cur = self.f.func.expect("current function");
        let name = format!("{}.stmts.{}", self.m.func(cur).name(), self.f.outlined);
        self.f.outlined += 1;
        let ptys = vec![LTy::PTR; args.len()];
        let fty = self.m.types.function(LTy::VOID, &ptys, false);
        let Ok(f) = self.m.add_function(&name, fty) else {
            for &s in stmts {
                self.emit_stmt(s)?;
            }
            return Ok(());
        };
        let pa = self.m.attr_set(&[Attr::Flag(AttrKind::NonNull), Attr::Flag(AttrKind::NoUndef)]);
        {
            let func = self.m.func_mut(f);
            func.linkage = Linkage::Internal;
            func.attrs.params = vec![pa; ptys.len()];
        }
        let decl = self.f.decl;
        let this_class = self.f.this_class;
        let saved = std::mem::replace(&mut self.f, Fx::empty());
        let r = self.emit_outlined_body(f, ptys, stmts, &cap, decl, this_class);
        self.f = saved;
        let noreturn = r?;
        let opts = CallOpts::default();
        self.emit_call_op(fty, V::C(Value::func(f)), args, opts, LTy::VOID, Some(f), false);
        if noreturn {
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            self.f.fir.set_cur(None);
        }
        Ok(())
    }

    /// `StartFunction`, the statements and `FinishFunction` of helper
    /// `f`; whether no path returns.
    fn emit_outlined_body(
        &mut self,
        f: FuncId,
        ptys: Vec<LTy>,
        stmts: &[StmtId],
        cap: &Captures,
        decl: Option<DeclId>,
        this_class: Option<DeclId>,
    ) -> R<bool> {
        self.outlined_prologue(f, ptys, cap, decl, this_class);
        let ret_block = self.f.fir.create_block();
        self.f.ret_block = Some(ret_block);
        self.f.ret_dest = Some(JumpDest { blk: ret_block, depth: Some(0), index: 1 });
        self.f.cl.next_index = 2;
        self.f.cl.prologue = self.f.cl.depth();
        for &s in stmts {
            self.emit_stmt(s)?;
        }
        let noreturn = self.f.fir.cur().is_none();
        let prologue = self.f.cl.prologue;
        self.pop_cleanup_blocks(prologue);
        self.emit_return_block();
        self.emit_epilog();
        self.outlined_finish(f, decl, noreturn)
    }

    /// The start of helper `f`'s body: the captured locals, `this` and
    /// the closure object bound to its parameters (in that order; any
    /// further parameter is the caller's to bind); the number of
    /// parameters bound.
    fn outlined_prologue(
        &mut self,
        f: FuncId,
        ptys: Vec<LTy>,
        cap: &Captures,
        decl: Option<DeclId>,
        this_class: Option<DeclId>,
    ) -> u32 {
        self.f.fir = Fir::new(ptys);
        self.start_fp();
        self.f.func = Some(f);
        self.f.decl = decl;
        self.f.this_class = this_class;
        let mut arg = 0u32;
        for &(d, a) in &cap.locals {
            self.f.locals.insert(d, Addr { v: V::Arg(arg), ..a });
            arg += 1;
        }
        if cap.this.is_some() {
            self.f.this = Some(V::Arg(arg));
            arg += 1;
        }
        if let Some((_, l)) = cap.closure {
            self.f.closure = Some((V::Arg(arg), l));
            arg += 1;
        }
        arg
    }

    /// `FinishFunction` of helper `f` after its return block: the EH
    /// blocks, the attributes and the lowering; `noreturn` is passed
    /// through.
    fn outlined_finish(&mut self, f: FuncId, decl: Option<DeclId>, noreturn: bool) -> R<bool> {
        self.finish_eh();
        self.emit_unreachable_if_used();
        if let Some(e) = self.f.error.take() {
            return Err(e);
        }
        if let Some(rb) = self.f.ret_block
            && self.f.fir.is_placed(rb)
            && self.f.fir.block_uses(rb) == 0
        {
            self.f.fir.erase_block(rb);
            self.f.ret_block = None;
        }
        let nounwind = !self.opts.exceptions || self.no_throw_calls();
        let mut extra = Vec::new();
        if let Some(d) = decl {
            self.def_opt_attrs(d, DefInline::NoInline, &mut extra);
        }
        let fa = self.def_fn_attrs(DefFnAttrs {
            decl,
            is_main: false,
            nounwind,
            mustprogress: self.f.mustprogress,
            vector_width: self.f.vector_width,
            noreturn,
            inline: DefInline::NoInline,
            extra,
        });
        self.m.func_mut(f).attrs.func = fa;
        self.nounwind.insert(f, nounwind);
        let fir = std::mem::replace(&mut self.f.fir, Fir::new(Vec::new()));
        let loc = decl.map(|d| self.ast.decls.get(d).loc());
        if self.f.uses_ph {
            let Some(d) = decl else {
                return crate::module::err(loc, "outlined statements outside a function");
            };
            self.pending.push((f, d, fir));
            return Ok(noreturn);
        }
        if let Err(e) = fir.lower(&mut self.m, f, &self.phs) {
            return crate::module::err(loc, format!("IR lowering failed: {e:?}"));
        }
        Ok(noreturn)
    }

    // ---- expansion statements (WP-793) ---------------------------------

    /// The elements of an expanded expansion statement to emit into
    /// helpers (`Some` per element to outline), or `None` to emit them
    /// all inline: the elements total at least `-fkiln-outline-expansions`
    /// statements and expressions, and the function may outline.
    pub(crate) fn expansion_plan(&self, elems: &[StmtId]) -> Option<Vec<Option<ElemScan>>> {
        let min = self.opts.outline_expansions;
        if min == 0 || elems.len() < 2 || !self.outline_enabled() {
            return None;
        }
        let scans: Vec<ElemScan> = elems.iter().map(|&e| self.scan_element(e)).collect();
        if scans.iter().map(|x| x.size).sum::<usize>() < min {
            return None;
        }
        let plan: Vec<Option<ElemScan>> =
            scans.into_iter().map(|x| (x.ok && x.size >= MIN_ELEMENT).then_some(x)).collect();
        plan.iter().any(Option::is_some).then_some(plan)
    }

    /// Scans expansion element `e` (a compound statement).
    fn scan_element(&self, e: StmtId) -> ElemScan {
        let mut sc = ElemScan { ok: true, size: 0, brk: false, ret: false, exprs: Vec::new() };
        self.scan_stmt(e, false, false, &mut sc);
        if sc.ok {
            let mut size = 0;
            sc.ok = sc.exprs.iter().all(|&x| self.expr_outlinable(x, &mut size).is_some());
            sc.size += size;
        }
        sc
    }

    /// Scans statement `s` of an expansion element: `nested` inside a
    /// loop or `switch` of the element (the target of a `break`),
    /// `in_switch` inside a `switch` of it (the owner of a `case`). An
    /// element a helper cannot emit has a label, a `goto`, a `case` of
    /// an enclosing `switch`, a coroutine, `asm` or contract statement,
    /// a dependent expansion statement, or a `return` of a named return
    /// value whose destruction the caller guards.
    fn scan_stmt(&self, s: StmtId, nested: bool, in_switch: bool, sc: &mut ElemScan) {
        if !sc.ok {
            return;
        }
        let ast = self.ast;
        let st = &ast.stmts;
        sc.size += 1;
        let n = *st.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => {
                for c in ast.lists.iter::<StmtId>(st.compound_body(s)) {
                    self.scan_stmt(c, nested, in_switch, sc);
                }
            }
            StmtKind::DeclStmt => {
                for d in ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)) {
                    self.scan_var(Some(d), sc);
                }
            }
            StmtKind::ExprStmt => sc.exprs.push(st.expr_stmt_expr(s)),
            StmtKind::NullStmt | StmtKind::ContinueStmt => {}
            StmtKind::BreakStmt => sc.brk |= !nested,
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                self.scan_opt(d.init, nested, in_switch, sc);
                self.scan_var(d.cond_var, sc);
                sc.exprs.extend(d.cond);
                self.scan_opt(d.then, nested, in_switch, sc);
                self.scan_opt(d.els, nested, in_switch, sc);
            }
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                self.scan_opt(d.init, nested, in_switch, sc);
                self.scan_var(d.cond_var, sc);
                sc.exprs.push(d.cond);
                self.scan_stmt(d.body, true, true, sc);
            }
            StmtKind::CaseStmt if in_switch => {
                let d = *st.case_data(s);
                sc.exprs.push(d.lhs);
                sc.exprs.extend(d.rhs);
                self.scan_stmt(d.sub, nested, in_switch, sc);
            }
            StmtKind::DefaultStmt if in_switch => {
                self.scan_stmt(st.default_sub(s), nested, in_switch, sc);
            }
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let d = *st.loop_data(s);
                self.scan_opt(d.init, nested, in_switch, sc);
                self.scan_var(d.cond_var, sc);
                sc.exprs.extend(d.cond);
                sc.exprs.extend(d.inc);
                self.scan_opt(d.body, true, in_switch, sc);
            }
            StmtKind::CXXForRangeStmt => {
                let d = *st.range_for_data(s);
                self.scan_opt(d.init, nested, in_switch, sc);
                for v in [Some(d.range), d.begin, d.end, Some(d.loop_var)] {
                    self.scan_var(v, sc);
                }
                sc.exprs.extend(d.cond);
                sc.exprs.extend(d.inc);
                self.scan_stmt(d.body, true, in_switch, sc);
            }
            StmtKind::CXXExpansionStmt if st.expansion_expanded(s) => {
                let d = *st.expansion_data(s);
                self.scan_opt(d.init, nested, in_switch, sc);
                for p in ast.lists.iter::<StmtId>(d.prelude) {
                    self.scan_stmt(p, nested, in_switch, sc);
                }
                for e in ast.lists.iter::<StmtId>(d.expansions) {
                    self.scan_stmt(e, true, in_switch, sc);
                }
            }
            StmtKind::ReturnStmt => {
                sc.ret = true;
                sc.exprs.extend(st.return_value(s));
                if let Some(v) = st.return_nrvo(s)
                    && self.f.nrvo_flags.contains_key(&v)
                {
                    sc.ok = false;
                }
            }
            StmtKind::AttributedStmt => {
                use kiln_ast::attr::{AttrKind as A, AttrNode};
                for a in ast.lists.iter::<kiln_ast::AttrId>(st.stmt_attrs(s)) {
                    let node = *ast.attrs.get(a);
                    if node.kind == A::ASSUME && node.flags & AttrNode::ASSUME_IGNORED == 0 {
                        sc.exprs.extend(<ExprId as kiln_ast::ids::RawId>::from_raw(node.a));
                    }
                }
                self.scan_stmt(st.sub_stmt(s), nested, in_switch, sc);
            }
            StmtKind::CXXTryStmt => {
                self.scan_stmt(st.try_block(s), nested, in_switch, sc);
                for h in ast.lists.iter::<StmtId>(st.try_handlers(s)) {
                    self.scan_stmt(st.catch_block(h), nested, in_switch, sc);
                }
            }
            _ => sc.ok = false,
        }
    }

    fn scan_opt(&self, s: Option<StmtId>, nested: bool, in_switch: bool, sc: &mut ElemScan) {
        if let Some(s) = s {
            self.scan_stmt(s, nested, in_switch, sc);
        }
    }

    /// The initializers of a variable declared in an expansion element
    /// (and of its tuple-like bindings' holding variables).
    fn scan_var(&self, d: Option<DeclId>, sc: &mut ElemScan) {
        let ast = self.ast;
        let Some(d) = d else { return };
        if ast.decls.get(d).kind() != DeclKind::Var {
            return;
        }
        sc.size += 1;
        sc.exprs.extend(ast.decls.var(d).and_then(|v| v.init));
        if let Some(l) = ast.decls.decomp_bindings(d) {
            for b in ast.lists.iter::<DeclId>(l) {
                if let Some(h) = ast.decls.binding_var(b) {
                    sc.exprs.extend(ast.decls.var(h).and_then(|v| v.init));
                }
            }
        }
    }

    /// Emits expansion element `e` into a new helper and calls it; `end`
    /// is the expansion statement's `break` destination. The helper
    /// returns an `i32` when the element breaks or returns
    /// ([`ELEM_NEXT`], [`ELEM_BREAK`], [`ELEM_RETURN`]), which the caller
    /// switches on, branching through its own cleanups; a value the
    /// element returns is stored through the caller's return slot,
    /// passed last. Whether it was outlined (else nothing is emitted).
    pub(crate) fn emit_outlined_element(
        &mut self,
        e: StmtId,
        sc: &ElemScan,
        end: JumpDest,
    ) -> R<bool> {
        let Some(cur) = self.f.func else { return Ok(false) };
        let ret_ok = !sc.ret
            || (self.f.ret_dest.is_some()
                && (self.f.ret_kind == RetKind::Void || self.f.ret_slot.is_some()));
        if self.f.fir.cur().is_none() || !ret_ok {
            return Ok(false);
        }
        let cap = self.captures_of(&sc.exprs);
        let status = sc.brk || sc.ret;
        let ret_slot = if sc.ret { self.f.ret_slot } else { None };
        let mut args: Vec<V> = cap.locals.iter().map(|&(_, a)| a.v).collect();
        args.extend(cap.this);
        args.extend(cap.closure.map(|(v, _)| v));
        args.extend(ret_slot.map(|a| a.v));
        let name = format!("{}.expand.{}", self.m.func(cur).name(), self.f.outlined);
        let ptys = vec![LTy::PTR; args.len()];
        let rty = if status { LTy::I32 } else { LTy::VOID };
        let fty = self.m.types.function(rty, &ptys, false);
        let Ok(f) = self.m.add_function(&name, fty) else { return Ok(false) };
        self.f.outlined += 1;
        let pa = self.m.attr_set(&[Attr::Flag(AttrKind::NonNull), Attr::Flag(AttrKind::NoUndef)]);
        {
            let func = self.m.func_mut(f);
            func.linkage = Linkage::Internal;
            func.attrs.params = vec![pa; ptys.len()];
        }
        let ret = (self.f.ret_kind, self.f.sig.clone(), ret_slot);
        let decl = self.f.decl;
        let this_class = self.f.this_class;
        let saved = std::mem::replace(&mut self.f, Fx::empty());
        let r = self.emit_element_body(f, ptys, e, &cap, decl, this_class, (sc.brk, sc.ret), ret);
        self.f = saved;
        let noreturn = r?;
        let opts = CallOpts::default();
        let rv = self.emit_call_op(fty, V::C(Value::func(f)), args, opts, rty, Some(f), false);
        if noreturn {
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            self.f.fir.set_cur(None);
            return Ok(true);
        }
        if status {
            let next = self.f.fir.create_block();
            let brk = sc.brk.then(|| self.f.fir.create_block());
            let ret = sc.ret.then(|| self.f.fir.create_block());
            let mut cases = Vec::new();
            for (b, k) in [(brk, ELEM_BREAK), (ret, ELEM_RETURN)] {
                if let Some(b) = b {
                    cases.push((V::C(self.m.const_int(LTy::I32, k)), b));
                }
            }
            self.f.fir.push(Op::Switch { c: rv, default: next, cases }, LTy::VOID);
            if let Some(b) = brk {
                self.emit_block(b, false);
                self.branch_through_cleanup(end);
            }
            if let Some(b) = ret {
                self.emit_block(b, false);
                if let Some(rd) = self.f.ret_dest {
                    self.branch_through_cleanup(rd);
                }
            }
            self.emit_block(next, false);
        }
        Ok(true)
    }

    /// The body of element helper `f` (see [`Self::emit_outlined_element`]):
    /// `exits` says whether the element breaks and returns, `ret` is the
    /// caller's return kind, signature and slot. Whether no path returns.
    #[allow(clippy::too_many_arguments, reason = "one call site, the helper's whole context")]
    fn emit_element_body(
        &mut self,
        f: FuncId,
        ptys: Vec<LTy>,
        e: StmtId,
        cap: &Captures,
        decl: Option<DeclId>,
        this_class: Option<DeclId>,
        exits: (bool, bool),
        ret: (RetKind, Option<Sig>, Option<Addr>),
    ) -> R<bool> {
        let arg = self.outlined_prologue(f, ptys, cap, decl, this_class);
        let (kind, sig, slot) = ret;
        self.f.ret_kind = kind;
        self.f.sig = sig;
        self.f.ret_slot = slot.map(|a| Addr { v: V::Arg(arg), ..a });
        self.f.cl.next_index = 1;
        self.f.cl.prologue = self.f.cl.depth();
        let next = self.jump_dest_here();
        let brk = self.jump_dest_here();
        let retd = self.jump_dest_here();
        self.f.ret_dest = Some(retd);
        self.f.jumps.push(Jump { brk, cont: Some(next) });
        let r = self.emit_stmt(e);
        self.f.jumps.pop();
        r?;
        let prologue = self.f.cl.prologue;
        self.pop_cleanup_blocks(prologue);
        let status = exits.0 || exits.1;
        if (self.f.fir.block_uses(brk.blk) != 0 && !exits.0)
            || (self.f.fir.block_uses(retd.blk) != 0 && !exits.1)
        {
            let loc = self.ast.stmts.get(e).loc();
            return crate::module::err(Some(loc), "an expansion element left by an unscanned jump");
        }
        let mut returns = false;
        for (d, k) in [(next, ELEM_NEXT), (brk, ELEM_BREAK), (retd, ELEM_RETURN)] {
            // The fall-through path returns where it is when nothing
            // continues to the element's end.
            if !(k == ELEM_NEXT && self.f.fir.block_uses(d.blk) == 0) {
                self.emit_block(d.blk, true);
            }
            if self.f.fir.cur().is_some() {
                let v = status.then(|| V::C(self.m.const_int(LTy::I32, k)));
                self.f.fir.push(Op::Ret { v }, LTy::VOID);
                self.f.fir.set_cur(None);
                returns = true;
            }
        }
        self.outlined_finish(f, decl, !returns)
    }
}
