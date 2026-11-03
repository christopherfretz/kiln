//! Statement emission (spec/codegen.md §4): clang's `CodeGenFunction::EmitStmt`
//! family at `-O0`.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_llvm::{BinOp, Flags, IntPredicate, TypeId as LTy};

use crate::cleanup::{Depth, JumpDest};
use crate::fir::{Blk, Op, V};
use crate::func::{Jump, LoopFrame, RetKind, SwitchCtx};
use crate::module::{Cg, R, err};

/// A `RunCleanupsScope` / `LexicalScope` in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scope {
    depth: Depth,
    le: usize,
    /// The `CleanupDeactivationScope`'s `OldDeactivateCleanupStackSize`.
    deferred: usize,
    lexical: bool,
    /// The enclosing `CurrentCleanupScopeDepth`.
    old_cur: Depth,
    /// The enclosing `DidCallStackSave` (WP-183).
    old_stack_save: bool,
}

impl Cg<'_> {
    /// Emits a function body (clang `EmitFunctionBody`: a compound body
    /// without a lexical scope of its own).
    pub(crate) fn emit_body(&mut self, body: StmtId) -> R<()> {
        let ast = self.ast;
        match ast.stmts.get(body).kind() {
            StmtKind::CompoundStmt => return self.emit_compound_without_scope(body),
            StmtKind::CoroutineBodyStmt => return self.emit_coroutine_body(body),
            _ => {}
        }
        self.emit_stmt(body)
    }

    fn emit_compound_without_scope(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        for c in ast.lists.iter::<StmtId>(ast.stmts.compound_body(s)).collect::<Vec<_>>() {
            self.emit_stmt(c)?;
        }
        Ok(())
    }

    // ---- scopes --------------------------------------------------------

    /// Enters a `RunCleanupsScope` (or, with `lexical`, a `LexicalScope`).
    pub(crate) fn begin_scope(&mut self, lexical: bool) -> Scope {
        if lexical {
            self.f.lex_labels.push(Vec::new());
        }
        let depth = self.f.cl.depth();
        let old_cur = std::mem::replace(&mut self.f.cl.cur_scope, depth);
        let old_stack_save = std::mem::replace(&mut self.f.did_stack_save, false);
        Scope {
            depth,
            le: self.f.cl.lifetime_extended.len(),
            deferred: self.f.cl.deferred.len(),
            lexical,
            old_cur,
            old_stack_save,
        }
    }

    /// `RunCleanupsScope::requiresCleanups`.
    pub(crate) fn requires_cleanups(&self, s: &Scope) -> bool {
        self.f.cl.depth() != s.depth
    }

    /// `ForceCleanup` (and the scope's destructor): deactivates the
    /// deferred cleanups (`CleanupDeactivationScope::ForceDeactivate`),
    /// pops the cleanups pushed since `s` began, then pushes the
    /// lifetime-extended ones; a lexical scope rescopes its labels.
    pub(crate) fn end_scope(&mut self, s: Scope) {
        self.end_scope_reload(s, &mut []);
    }

    /// `ForceCleanup(ValuesToReload)`: [`Cg::end_scope`], keeping the
    /// values in `reload` usable at the insertion point when the popped
    /// cleanups are shared with a branch out of the scope (a `co_await`
    /// suspension's destroy path, a statement expression's jump).
    pub(crate) fn end_scope_reload(&mut self, s: Scope, reload: &mut [&mut V]) {
        let labels = if s.lexical { self.f.lex_labels.pop() } else { None };
        self.deactivate_deferred(s.deferred);
        self.pop_cleanup_blocks_le_reload(s.depth, s.le, reload);
        self.f.cl.cur_scope = s.old_cur;
        self.f.did_stack_save = s.old_stack_save;
        if let Some(labels) = labels
            && !labels.is_empty()
        {
            // rescopeLabels.
            let innermost = self.f.cl.innermost_normal();
            for l in &labels {
                if let Some(j) = self.f.labels.get_mut(l) {
                    j.depth = Some(innermost);
                }
            }
            if innermost != 0
                && let Some(parent) = self.f.lex_labels.last_mut()
            {
                parent.extend(labels);
            }
        }
    }

    /// `getJumpDestInCurrentScope` for an existing block.
    fn jump_dest_at(&mut self, blk: Blk) -> JumpDest {
        let index = self.f.cl.next_index;
        self.f.cl.next_index += 1;
        JumpDest { blk, depth: Some(self.f.cl.depth()), index }
    }

    /// `getJumpDestForLabel`.
    pub(crate) fn label_dest(&mut self, d: DeclId) -> JumpDest {
        if let Some(&j) = self.f.labels.get(&d) {
            return j;
        }
        let blk = self.f.fir.create_block();
        let index = self.f.cl.next_index;
        self.f.cl.next_index += 1;
        let j = JumpDest { blk, depth: None, index };
        self.f.labels.insert(d, j);
        j
    }

    /// clang `EmitLabel`.
    pub(crate) fn emit_label(&mut self, d: DeclId) {
        let blk = match self.f.labels.get(&d).copied() {
            None => {
                let b = self.f.fir.create_block();
                let j = self.jump_dest_at(b);
                self.f.labels.insert(d, j);
                b
            }
            Some(j) => {
                let depth: Depth = self.f.cl.depth();
                if let Some(x) = self.f.labels.get_mut(&d) {
                    x.depth = Some(depth);
                }
                self.resolve_branch_fixups(j.blk);
                j.blk
            }
        };
        if self.f.cl.has_normal_cleanups()
            && let Some(l) = self.f.lex_labels.last_mut()
        {
            l.push(d);
        }
        self.emit_block(blk, false);
    }

    fn stmt_children(&self, s: StmtId) -> Vec<StmtId> {
        let st = &self.ast.stmts;
        let n = *st.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => self.ast.lists.iter(st.compound_body(s)).collect(),
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                d.init.into_iter().chain(d.then).chain(d.els).collect()
            }
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                d.init.into_iter().chain(Some(d.body)).collect()
            }
            StmtKind::CaseStmt => vec![st.case_data(s).sub],
            StmtKind::DefaultStmt => vec![st.default_sub(s)],
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let d = *st.loop_data(s);
                d.init.into_iter().chain(d.body).collect()
            }
            StmtKind::CXXForRangeStmt => {
                let d = *st.range_for_data(s);
                d.init.into_iter().chain(Some(d.body)).collect()
            }
            StmtKind::LabelStmt | StmtKind::AttributedStmt => vec![StmtId::new(n.b())],
            _ => Vec::new(),
        }
    }

    /// clang `ContainsLabel` (with `IgnoreCaseStmts` = false).
    fn contains_label(&self, s: StmtId) -> bool {
        let k = self.ast.stmts.get(s).kind();
        if matches!(k, StmtKind::LabelStmt | StmtKind::CaseStmt | StmtKind::DefaultStmt) {
            return true;
        }
        if k == StmtKind::SwitchStmt {
            // Case labels inside a nested switch belong to it.
            return self.stmt_children(s).into_iter().any(|c| self.contains_label_no_case(c));
        }
        self.stmt_children(s).into_iter().any(|c| self.contains_label(c))
    }

    fn contains_label_no_case(&self, s: StmtId) -> bool {
        let k = self.ast.stmts.get(s).kind();
        if k == StmtKind::LabelStmt {
            return true;
        }
        self.stmt_children(s).into_iter().any(|c| self.contains_label_no_case(c))
    }

    /// clang `EmitStmt`.
    pub(crate) fn emit_stmt(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let n = *ast.stmts.get(s);
        let simple = matches!(
            n.kind(),
            StmtKind::NullStmt
                | StmtKind::CompoundStmt
                | StmtKind::DeclStmt
                | StmtKind::LabelStmt
                | StmtKind::AttributedStmt
                | StmtKind::GotoStmt
                | StmtKind::BreakStmt
                | StmtKind::ContinueStmt
                | StmtKind::CaseStmt
                | StmtKind::DefaultStmt
        );
        if !simple && self.f.fir.cur().is_none() {
            if !self.contains_label(s) {
                return Ok(());
            }
            self.ensure_insert_point();
        }
        match n.kind() {
            StmtKind::NullStmt => Ok(()),
            StmtKind::CompoundStmt => {
                let sc = self.begin_scope(true);
                self.emit_compound_without_scope(s)?;
                self.end_scope(sc);
                Ok(())
            }
            StmtKind::DeclStmt => {
                for d in ast.lists.iter::<DeclId>(ast.stmts.decl_stmt_decls(s)).collect::<Vec<_>>()
                {
                    match ast.decls.get(d).kind() {
                        DeclKind::Var => self.emit_var_decl(d)?,
                        // clang `EmitDecl`: a typedef of a variably
                        // modified type evaluates its bounds.
                        DeclKind::Typedef | DeclKind::TypeAlias => {
                            if let Some(t) = ast.decls.typedef(d) {
                                self.emit_vla_bounds(t.underlying)?;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            StmtKind::ExprStmt => {
                let incoming = self.f.fir.cur();
                self.emit_ignored(ast.stmts.expr_stmt_expr(s))?;
                // A noreturn call (`__builtin_unreachable()`) leaves the
                // insertion point in a fresh block nothing branches to:
                // drop it, so what follows is unreachable and skipped.
                if let Some(out) = self.f.fir.cur()
                    && Some(out) != incoming
                    && self.f.fir.block_uses(out) == 0
                    && out != self.f.fir.entry()
                {
                    self.f.fir.erase_block(out);
                }
                Ok(())
            }
            StmtKind::AttributedStmt => {
                self.emit_assumptions(s)?;
                self.emit_stmt(StmtId::new(n.b()))
            }
            StmtKind::LabelStmt => {
                self.emit_label(DeclId::new(n.a()));
                self.emit_stmt(StmtId::new(n.b()))
            }
            StmtKind::GotoStmt => {
                let j = self.label_dest(DeclId::new(n.a()));
                self.branch_through_cleanup(j);
                Ok(())
            }
            StmtKind::BreakStmt => {
                let j = self.f.jumps.last().expect("break outside a loop").brk;
                self.branch_through_cleanup(j);
                Ok(())
            }
            StmtKind::ContinueStmt => {
                let c = self.f.jumps.last().and_then(|j| j.cont).expect("continue outside a loop");
                self.branch_through_cleanup(c);
                Ok(())
            }
            StmtKind::ReturnStmt => self.emit_return(s),
            StmtKind::IfStmt => self.emit_if(s),
            StmtKind::WhileStmt => self.emit_while(s),
            StmtKind::DoStmt => self.emit_do(s),
            StmtKind::ForStmt => self.emit_for(s),
            StmtKind::CXXForRangeStmt => self.emit_range_for(s),
            StmtKind::SwitchStmt => self.emit_switch(s),
            StmtKind::CaseStmt => self.emit_case(s),
            StmtKind::DefaultStmt => {
                let sw = self.f.sw.expect("default outside a switch");
                self.emit_block(sw.default, false);
                self.emit_stmt(ast.stmts.default_sub(s))
            }
            StmtKind::CXXTryStmt => self.emit_try_stmt(s),
            StmtKind::GCCAsmStmt => self.emit_asm(s),
            StmtKind::CoreturnStmt => self.emit_coreturn(s),
            StmtKind::CoroutineBodyStmt => self.emit_coroutine_body(s),
            k => err(Some(n.loc()), format!("statement {k:?} is not supported yet")),
        }
    }

    fn emit_return(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let sc = self.begin_scope(false);
        let mut rv = ast.stmts.return_value(s);
        if let Some(e) = rv
            && ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups
        {
            rv = Some(self.sub(e));
        }
        let nrvo = ast
            .stmts
            .return_nrvo(s)
            .filter(|&v| ast.decls.var(v).is_some_and(|x| x.spec.is_nrvo()));
        if let Some(v) = nrvo {
            if let Some(&flag) = self.f.nrvo_flags.get(&v) {
                let t = V::C(self.m.const_bool(true));
                self.store(t, flag, 1, false);
            }
        } else if let Some(e) = rv {
            self.store_return_value(e)?;
        }
        self.end_scope(sc);
        let rd = self.f.ret_dest.expect("return destination");
        self.branch_through_cleanup(rd);
        Ok(())
    }

    /// Evaluates the returned expression `e` into the return slot
    /// (clang `EmitReturnStmt` without the branch).
    pub(crate) fn store_return_value(&mut self, e: ExprId) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let ret_ref = self.f.sig.as_ref().is_some_and(|g| self.referee(g.ret).is_some());
        if ast.types.is_void(q) || self.f.ret_slot.is_none() {
            self.emit_ignored(e)?;
        } else if ret_ref {
            let p = self.emit_ref_binding(e)?;
            let slot = self.f.ret_slot.expect("return slot");
            self.store(p, slot.v, slot.align, false);
        } else if self.is_aggregate(q) || self.is_atomic_aggregate(q) {
            let slot = self.f.ret_slot.expect("return slot");
            self.emit_agg_slot(e, slot, false, false)?;
        } else if self.is_complex(q) {
            let v = self.emit_complex(e)?;
            let slot = self.f.ret_slot.expect("return slot");
            self.cx_store(v, slot, q);
        } else {
            let v = self.emit_scalar(e)?;
            let slot = self.f.ret_slot.expect("return slot");
            if self.f.ret_kind == RetKind::Sret {
                self.store_scalar(v, slot, q);
            } else {
                self.store(v, slot.v, slot.align, false);
            }
        }
        Ok(())
    }

    fn emit_if(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.if_data(s);
        let kind = ast.stmts.if_kind(s);
        if matches!(kind, IfKind::Consteval | IfKind::NotConsteval) {
            let live = if kind == IfKind::Consteval { d.els } else { d.then };
            if let Some(l) = live {
                let sc = self.begin_scope(false);
                self.emit_stmt(l)?;
                self.end_scope(sc);
            }
            return Ok(());
        }
        let cond_scope = self.begin_scope(true);
        if let Some(i) = d.init {
            self.emit_stmt(i)?;
        }
        let deferred = match d.cond_var {
            Some(v) => self.emit_cond_var_decl(v)?.then_some(v),
            None => None,
        };
        let cond = d.cond.expect("if condition");
        if let Some(b) = self.fold_bool(cond).filter(|_| deferred.is_none()) {
            let (live, dead) = if b { (d.then, d.els) } else { (d.els, d.then) };
            if kind == IfKind::Constexpr || !dead.is_some_and(|x| self.contains_label(x)) {
                if let Some(l) = live {
                    let sc = self.begin_scope(false);
                    self.emit_stmt(l)?;
                    self.end_scope(sc);
                }
                self.end_scope(cond_scope);
                return Ok(());
            }
        }
        let then_b = self.f.fir.create_block();
        let end_b = self.f.fir.create_block();
        let else_b = if d.els.is_some() { self.f.fir.create_block() } else { end_b };
        if let Some(v) = deferred {
            let cv = self.eval_bool(cond)?;
            self.emit_binding_vars(v)?;
            self.cond_br(cv, then_b, else_b);
        } else {
            self.branch_on_bool(cond, then_b, else_b)?;
        }
        self.emit_block(then_b, false);
        if let Some(t) = d.then {
            let sc = self.begin_scope(false);
            self.emit_stmt(t)?;
            self.end_scope(sc);
        }
        self.emit_branch(end_b);
        if let Some(e) = d.els {
            self.emit_block(else_b, false);
            let sc = self.begin_scope(false);
            self.emit_stmt(e)?;
            self.end_scope(sc);
            self.emit_branch(end_b);
        }
        self.emit_block(end_b, true);
        self.end_scope(cond_scope);
        Ok(())
    }

    /// clang `EmitAttributedStmt`: `llvm.assume` of each `[[assume]]`
    /// assumption without possible side effects ([dcl.attr.assume]),
    /// when the point is reachable (even at `-O0`).
    fn emit_assumptions(&mut self, s: StmtId) -> R<()> {
        use kiln_ast::attr::{AttrKind, AttrNode};
        use kiln_ast::ids::RawId;
        let ast = self.ast;
        for a in ast.lists.iter::<kiln_ast::AttrId>(ast.stmts.stmt_attrs(s)) {
            let node = *ast.attrs.get(a);
            if node.kind != AttrKind::ASSUME
                || node.flags & AttrNode::ASSUME_IGNORED != 0
                || self.f.fir.cur().is_none()
            {
                continue;
            }
            let Some(e) = ExprId::from_raw(node.a) else { continue };
            let c = self.emit_scalar(e)?;
            let f = kiln_llvm::intrinsics::assume(&mut self.m);
            self.intrinsic_call(f, vec![c]);
        }
        Ok(())
    }

    fn is_empty_body(&self, body: Option<StmtId>) -> bool {
        match body {
            None => true,
            Some(b) => {
                let n = self.ast.stmts.get(b);
                match n.kind() {
                    StmtKind::NullStmt => true,
                    StmtKind::CompoundStmt => {
                        self.ast.lists.get(self.ast.stmts.compound_body(b)).is_empty()
                    }
                    _ => false,
                }
            }
        }
    }

    /// clang `checkIfLoopMustProgress` plus the `LoopAttributes` metadata.
    fn new_loop_md(
        &mut self,
        cond: Option<ExprId>,
        body: Option<StmtId>,
    ) -> Option<kiln_llvm::MetadataId> {
        let cond_true = match cond {
            None => true,
            Some(c) => self.fold_bool(c) == Some(true),
        };
        let mp = if cond_true && self.is_empty_body(body) {
            self.f.mustprogress = false;
            None
        } else if let Some(m) = self.f.loop_mp {
            Some(m)
        } else {
            let s = self.m.md_string(b"llvm.loop.mustprogress");
            let m = self.m.md_tuple(&[Some(s)]);
            self.f.loop_mp = Some(m);
            Some(m)
        };
        self.loop_id(mp)
    }

    /// clang `LoopInfo::createMetadata`: the distinct `llvm.loop` node
    /// `{self, mustprogress?, unroll.disable?}`, or `None` when it has no
    /// property. At `-O1` (clang `OptimizationLevel > 0` without
    /// `UnrollLoops`, which `-O2` and above set) every loop carries
    /// `llvm.loop.unroll.disable` (spec/codegen.md "Loop metadata").
    fn loop_id(&mut self, mp: Option<kiln_llvm::MetadataId>) -> Option<kiln_llvm::MetadataId> {
        let unroll = if self.opts.opt_level == 1 {
            Some(if let Some(u) = self.f.loop_unroll {
                u
            } else {
                let s = self.m.md_string(b"llvm.loop.unroll.disable");
                let u = self.m.md_tuple(&[Some(s)]);
                self.f.loop_unroll = Some(u);
                u
            })
        } else {
            None
        };
        if mp.is_none() && unroll.is_none() {
            return None;
        }
        let ops: Vec<_> =
            std::iter::once(None).chain([mp, unroll].into_iter().flatten().map(Some)).collect();
        let node = self.m.md_distinct_tuple(&ops);
        self.m.md_set_operand(node, 0, Some(node)).expect("distinct loop id");
        Some(node)
    }

    fn emit_while(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.loop_data(s);
        let header_b = self.f.fir.create_block();
        let header = self.jump_dest_at(header_b);
        self.emit_block(header_b, false);
        let end = self.jump_dest_here();
        self.f.jumps.push(Jump { brk: end, cont: Some(header) });
        let cond_scope = self.begin_scope(false);
        let deferred = match d.cond_var {
            Some(v) => self.emit_cond_var_decl(v)?.then_some(v),
            None => None,
        };
        let cond = d.cond.expect("while condition");
        let cv = self.eval_bool(cond)?;
        if let Some(v) = deferred {
            self.emit_binding_vars(v)?;
        }
        let const_true = matches!(self.cint(cv), Some((_, 1, _)));
        let md = self.new_loop_md(Some(cond), d.body);
        self.f.loops.push(LoopFrame { header: header_b, md });
        let body = self.f.fir.create_block();
        if !const_true {
            let exit = if self.requires_cleanups(&cond_scope) {
                self.f.fir.create_block()
            } else {
                end.blk
            };
            self.cond_br(cv, body, exit);
            if exit != end.blk {
                self.emit_block(exit, false);
                self.branch_through_cleanup(end);
            }
        }
        let body_scope = self.begin_scope(false);
        self.emit_block(body, false);
        if let Some(b) = d.body {
            self.emit_stmt(b)?;
        }
        self.end_scope(body_scope);
        self.f.jumps.pop();
        self.end_scope(cond_scope);
        self.emit_branch(header_b);
        self.f.loops.pop();
        self.emit_block(end.blk, true);
        if const_true {
            self.simplify_forwarding(header_b);
        }
        Ok(())
    }

    fn emit_do(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.loop_data(s);
        let end = self.jump_dest_here();
        let cond_d = self.jump_dest_here();
        self.f.jumps.push(Jump { brk: end, cont: Some(cond_d) });
        let body_b = self.f.fir.create_block();
        self.emit_block(body_b, false);
        if let Some(b) = d.body {
            let sc = self.begin_scope(false);
            self.emit_stmt(b)?;
            self.end_scope(sc);
        }
        self.emit_block(cond_d.blk, false);
        let cond = d.cond.expect("do condition");
        let cv = self.eval_bool(cond)?;
        self.f.jumps.pop();
        let const_false = matches!(self.cint(cv), Some((_, 0, _)));
        let md = self.new_loop_md(Some(cond), d.body);
        self.f.loops.push(LoopFrame { header: body_b, md });
        if !const_false {
            self.cond_br(cv, body_b, end.blk);
        }
        self.f.loops.pop();
        self.emit_block(end.blk, false);
        if const_false {
            self.simplify_forwarding(cond_d.blk);
        }
        Ok(())
    }

    fn emit_for(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.loop_data(s);
        let end = self.jump_dest_here();
        let for_scope = self.begin_scope(true);
        if let Some(i) = d.init {
            self.emit_stmt(i)?;
        }
        let cond_d = self.jump_dest_here();
        let cond_b = cond_d.blk;
        self.emit_block(cond_b, false);
        let md = self.new_loop_md(d.cond, d.body);
        self.f.loops.push(LoopFrame { header: cond_b, md });
        let cond_scope = self.begin_scope(true);
        let mut cont = if d.inc.is_none() {
            Some(cond_d)
        } else if d.cond_var.is_none() {
            Some(self.jump_dest_here())
        } else {
            None
        };
        self.f.jumps.push(Jump { brk: end, cont });
        if let Some(c) = d.cond {
            let deferred = match d.cond_var {
                Some(v) => self.emit_cond_var_decl(v)?.then_some(v),
                None => None,
            };
            if d.cond_var.is_some() {
                cont = Some(if d.inc.is_some() { self.jump_dest_here() } else { cond_d });
                self.f.jumps.last_mut().expect("loop").cont = cont;
            }
            let cv = self.eval_bool(c)?;
            if let Some(v) = deferred {
                self.emit_binding_vars(v)?;
            }
            let exit = if self.requires_cleanups(&for_scope) {
                self.f.fir.create_block()
            } else {
                end.blk
            };
            let body_b = self.f.fir.create_block();
            self.cond_br(cv, body_b, exit);
            if exit != end.blk {
                self.emit_block(exit, false);
                self.branch_through_cleanup(end);
            }
            self.emit_block(body_b, false);
        }
        if let Some(b) = d.body {
            let sc = self.begin_scope(false);
            self.emit_stmt(b)?;
            self.end_scope(sc);
        }
        if let Some(inc) = d.inc {
            let c = cont.expect("continue destination");
            self.emit_block(c.blk, false);
            self.emit_ignored(inc)?;
        }
        self.f.jumps.pop();
        self.end_scope(cond_scope);
        self.emit_branch(cond_b);
        self.end_scope(for_scope);
        self.f.loops.pop();
        self.emit_block(end.blk, true);
        Ok(())
    }

    /// clang `EmitCXXForRangeStmt`: the implicit variables, then the
    /// loop. clang pushes the loop without `mustprogress`, so it carries
    /// `llvm.loop` metadata only at `-O1` (`unroll.disable`).
    fn emit_range_for(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.range_for_data(s);
        let loc = ast.stmts.get(s).loc();
        let (Some(begin), Some(end_var), Some(cond), Some(inc)) = (d.begin, d.end, d.cond, d.inc)
        else {
            return err(Some(loc), "a dependent range-based for statement");
        };
        let end = self.jump_dest_here();
        let for_scope = self.begin_scope(true);
        if let Some(i) = d.init {
            self.emit_stmt(i)?;
        }
        self.emit_var_decl(d.range)?;
        self.emit_var_decl(begin)?;
        self.emit_var_decl(end_var)?;
        let cond_d = self.jump_dest_here();
        let cond_b = cond_d.blk;
        self.emit_block(cond_b, false);
        let md = self.loop_id(None);
        self.f.loops.push(LoopFrame { header: cond_b, md });
        let exit =
            if self.requires_cleanups(&for_scope) { self.f.fir.create_block() } else { end.blk };
        let body_b = self.f.fir.create_block();
        let cv = self.eval_bool(cond)?;
        self.cond_br(cv, body_b, exit);
        if exit != end.blk {
            self.emit_block(exit, false);
            self.branch_through_cleanup(end);
        }
        self.emit_block(body_b, false);
        let next = self.jump_dest_here();
        self.f.jumps.push(Jump { brk: end, cont: Some(next) });
        let sc = self.begin_scope(false);
        self.emit_var_decl(d.loop_var)?;
        self.emit_stmt(d.body)?;
        self.end_scope(sc);
        self.emit_block(next.blk, false);
        self.emit_ignored(inc)?;
        self.f.jumps.pop();
        self.emit_branch(cond_b);
        self.end_scope(for_scope);
        self.f.loops.pop();
        self.emit_block(end.blk, true);
        Ok(())
    }

    fn emit_switch(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.switch_data(s);
        let epilog = self.jump_dest_here();
        let cond_scope = self.begin_scope(false);
        if let Some(i) = d.init {
            self.emit_stmt(i)?;
        }
        // [stmt.pre]/4 (P0963): a structured binding declaration's
        // tuple-like bindings are initialized after the condition.
        let deferred = match d.cond_var {
            Some(v) => self.emit_cond_var_decl(v)?.then_some(v),
            None => None,
        };
        let c = self.emit_scalar(d.cond)?;
        if let Some(v) = deferred {
            self.emit_binding_vars(v)?;
        }
        let cond_ty = self.f.fir.ty(&self.m, c);
        let default = self.f.fir.create_block();
        let sw = self.f.fir.push(Op::Switch { c, default, cases: Vec::new() }, LTy::VOID);
        self.f.fir.set_cur(None);
        let outer_cont = self.f.jumps.last().and_then(|j| j.cont);
        self.f.jumps.push(Jump { brk: epilog, cont: outer_cont });
        let inst = crate::fir::Fir::last_inst(sw);
        let ctx = SwitchCtx { inst, cond: c, cond_ty, default, range_head: default };
        let saved = self.f.sw.replace(ctx);
        self.emit_stmt(d.body)?;
        self.f.jumps.pop();
        // `setDefaultDest(CaseRangeBlock)`.
        let head = self.f.sw.expect("switch").range_head;
        if head != default {
            self.f.fir.set_successor(inst, default, head);
        }
        if !self.f.fir.is_placed(default) {
            if self.requires_cleanups(&cond_scope) {
                self.emit_block(default, false);
            } else {
                self.f.fir.replace_block_uses(default, epilog.blk);
            }
        }
        self.end_scope(cond_scope);
        self.emit_block(epilog.blk, true);
        self.f.sw = saved;
        Ok(())
    }

    fn emit_case(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let mut s = s;
        let b = self.f.fir.create_block();
        self.emit_block(b, false);
        loop {
            let cd = *ast.stmts.case_data(s);
            if let Some(rhs) = cd.rhs {
                return self.emit_case_range(cd.lhs, rhs, cd.sub);
            }
            let sw = self.f.sw.expect("case outside a switch");
            let v = self.fold_int(cd.lhs).expect("case value folds");
            let k = V::C(self.m.const_int(sw.cond_ty, v));
            self.f.fir.add_switch_case(sw.inst, k, b);
            if ast.stmts.get(cd.sub).kind() == StmtKind::CaseStmt
                && ast.stmts.case_data(cd.sub).rhs.is_none()
            {
                s = cd.sub;
                continue;
            }
            return self.emit_stmt(cd.sub);
        }
    }

    /// clang `EmitCaseStmtRange`: the block of `case lo ... hi: sub`
    /// (already started by [`Self::emit_case`]) and its sub-statement,
    /// then a switch case per value when the range has fewer than 64
    /// more, else an unsigned range test chained before the default.
    fn emit_case_range(&mut self, lo: ExprId, hi: ExprId, sub: StmtId) -> R<()> {
        let dest = self.f.fir.cur().expect("case block");
        self.emit_stmt(sub)?;
        let mut sw = self.f.sw.expect("case outside a switch");
        let lo = self.fold_int(lo).expect("case value folds");
        let hi = self.fold_int(hi).expect("case value folds");
        if hi < lo {
            return Ok(());
        }
        let range = hi - lo;
        if range < 64 {
            for v in lo..=hi {
                let k = V::C(self.m.const_int(sw.cond_ty, v));
                self.f.fir.add_switch_case(sw.inst, k, dest);
            }
            return Ok(());
        }
        let restore = self.f.fir.cur();
        let false_dest = sw.range_head;
        let b = self.f.fir.create_block();
        sw.range_head = b;
        self.f.sw = Some(sw);
        self.f.fir.place_at_end(b);
        self.f.fir.set_cur(Some(b));
        let lv = V::C(self.m.const_int(sw.cond_ty, lo));
        let diff = self.bin(BinOp::Sub, Flags::NONE, sw.cond, lv);
        let rv = V::C(self.m.const_int(sw.cond_ty, range));
        let inbounds = self.icmp(IntPredicate::Ule, diff, rv);
        self.cond_br(inbounds, dest, false_dest);
        self.f.fir.set_cur(restore);
        Ok(())
    }
}
