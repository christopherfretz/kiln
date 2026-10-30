//! Statement emission (spec/codegen.md §4): clang's `CodeGenFunction::EmitStmt`
//! family at `-O0`.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_llvm::TypeId as LTy;

use crate::cleanup::{Depth, JumpDest};
use crate::fir::{Blk, Op, V};
use crate::func::{Jump, LoopFrame, RetKind, SwitchCtx};
use crate::module::{Cg, R, err};

/// A `RunCleanupsScope` / `LexicalScope` in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scope {
    depth: Depth,
    le: usize,
    lexical: bool,
}

impl Cg<'_> {
    /// Emits a function body (clang `EmitFunctionBody`: a compound body
    /// without a lexical scope of its own).
    pub(crate) fn emit_body(&mut self, body: StmtId) -> R<()> {
        let ast = self.ast;
        if ast.stmts.get(body).kind() == StmtKind::CompoundStmt {
            return self.emit_compound_without_scope(body);
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
        Scope { depth: self.f.cl.depth(), le: self.f.cl.lifetime_extended.len(), lexical }
    }

    /// `RunCleanupsScope::requiresCleanups`.
    pub(crate) fn requires_cleanups(&self, s: &Scope) -> bool {
        self.f.cl.depth() != s.depth
    }

    /// `ForceCleanup` (and the scope's destructor): pops the cleanups
    /// pushed since `s` began, then pushes the lifetime-extended ones;
    /// a lexical scope rescopes its labels.
    pub(crate) fn end_scope(&mut self, s: Scope) {
        let labels = if s.lexical { self.f.lex_labels.pop() } else { None };
        self.pop_cleanup_blocks_le(s.depth, s.le);
        if let Some(labels) = labels
            && !labels.is_empty()
        {
            // rescopeLabels.
            let innermost = self.f.cl.depth();
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
    fn label_dest(&mut self, d: DeclId) -> JumpDest {
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
        if !self.f.cl.stack.is_empty()
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
                    if ast.decls.get(d).kind() == DeclKind::Var {
                        self.emit_var_decl(d)?;
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
            StmtKind::AttributedStmt => self.emit_stmt(StmtId::new(n.b())),
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
            StmtKind::SwitchStmt => self.emit_switch(s),
            StmtKind::CaseStmt => self.emit_case(s),
            StmtKind::DefaultStmt => {
                let sw = self.f.sw.expect("default outside a switch");
                self.emit_block(sw.default, false);
                self.emit_stmt(ast.stmts.default_sub(s))
            }
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
        let nrvo = ast.stmts.return_nrvo(s).filter(|&v| {
            ast.decls.var(v).is_some_and(|x| x.spec.is_nrvo())
        });
        if let Some(v) = nrvo {
            if let Some(&flag) = self.f.nrvo_flags.get(&v) {
                let t = V::C(self.m.const_bool(true));
                self.store(t, flag, 1, false);
            }
        } else if let Some(e) = rv {
            let q = ast.exprs.get(e).ty();
            let ret_ref = self.f.sig.as_ref().is_some_and(|g| self.referee(g.ret).is_some());
            if ast.types.is_void(q) || self.f.ret_slot.is_none() {
                self.emit_ignored(e)?;
            } else if ret_ref {
                let p = self.emit_ref_binding(e)?;
                let slot = self.f.ret_slot.expect("return slot");
                self.store(p, slot.v, slot.align, false);
            } else if self.is_aggregate(q) {
                let slot = self.f.ret_slot.expect("return slot");
                self.emit_agg_slot(e, slot, false, false)?;
            } else {
                let v = self.emit_scalar(e)?;
                let slot = self.f.ret_slot.expect("return slot");
                if self.f.ret_kind == RetKind::Sret {
                    self.store_scalar(v, slot, q);
                } else {
                    self.store(v, slot.v, slot.align, false);
                }
            }
        }
        self.end_scope(sc);
        let rd = self.f.ret_dest.expect("return destination");
        self.branch_through_cleanup(rd);
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
        if let Some(v) = d.cond_var {
            self.emit_var_decl(v)?;
        }
        let cond = d.cond.expect("if condition");
        if let Some(b) = self.fold_bool(cond) {
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
        self.branch_on_bool(cond, then_b, else_b)?;
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
        if cond_true && self.is_empty_body(body) {
            self.f.mustprogress = false;
            return None;
        }
        let mp = if let Some(m) = self.f.loop_mp {
            m
        } else {
            let s = self.m.md_string(b"llvm.loop.mustprogress");
            let m = self.m.md_tuple(&[Some(s)]);
            self.f.loop_mp = Some(m);
            m
        };
        let node = self.m.md_distinct_tuple(&[None, Some(mp)]);
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
        if let Some(v) = d.cond_var {
            self.emit_var_decl(v)?;
        }
        let cond = d.cond.expect("while condition");
        let cv = self.eval_bool(cond)?;
        let const_true = matches!(self.cint(cv), Some((_, 1, _)));
        let md = self.new_loop_md(Some(cond), d.body);
        self.f.loops.push(LoopFrame { header: header_b, md });
        let body = self.f.fir.create_block();
        if !const_true {
            let exit =
                if self.requires_cleanups(&cond_scope) { self.f.fir.create_block() } else { end.blk };
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
            if let Some(v) = d.cond_var {
                self.emit_var_decl(v)?;
                cont = Some(if d.inc.is_some() { self.jump_dest_here() } else { cond_d });
                self.f.jumps.last_mut().expect("loop").cont = cont;
            }
            let exit =
                if self.requires_cleanups(&for_scope) { self.f.fir.create_block() } else { end.blk };
            let body_b = self.f.fir.create_block();
            let cv = self.eval_bool(c)?;
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

    fn emit_switch(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.switch_data(s);
        let epilog = self.jump_dest_here();
        let cond_scope = self.begin_scope(false);
        if let Some(i) = d.init {
            self.emit_stmt(i)?;
        }
        if let Some(v) = d.cond_var {
            self.emit_var_decl(v)?;
        }
        let c = self.emit_scalar(d.cond)?;
        let cond_ty = self.f.fir.ty(&self.m, c);
        let default = self.f.fir.create_block();
        let sw = self.f.fir.push(Op::Switch { c, default, cases: Vec::new() }, LTy::VOID);
        self.f.fir.set_cur(None);
        let outer_cont = self.f.jumps.last().and_then(|j| j.cont);
        self.f.jumps.push(Jump { brk: epilog, cont: outer_cont });
        let saved =
            self.f.sw.replace(SwitchCtx { inst: crate::fir::Fir::last_inst(sw), cond_ty, default });
        self.emit_stmt(d.body)?;
        self.f.jumps.pop();
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
            if cd.rhs.is_some() {
                return err(Some(ast.stmts.get(s).loc()), "case ranges are not supported yet");
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
}
