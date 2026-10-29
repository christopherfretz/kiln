//! Statement emission (spec/codegen.md §4): clang's `CodeGenFunction::EmitStmt`
//! family at `-O0`.

use kiln_ast::decl::DeclKind;
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_llvm::TypeId as LTy;

use crate::fir::{Blk, Op, V};
use crate::func::{Jump, LoopFrame, RetKind, SwitchCtx};
use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// Emits a function body.
    pub(crate) fn emit_body(&mut self, body: StmtId) -> R<()> {
        self.emit_stmt(body)
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
                for c in ast.lists.iter::<StmtId>(ast.stmts.compound_body(s)).collect::<Vec<_>>() {
                    self.emit_stmt(c)?;
                }
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
                let b = self.label_block(DeclId::new(n.a()));
                self.emit_block(b, false);
                self.emit_stmt(StmtId::new(n.b()))
            }
            StmtKind::GotoStmt => {
                let b = self.label_block(DeclId::new(n.a()));
                if self.f.fir.cur().is_some() {
                    self.emit_branch(b);
                }
                Ok(())
            }
            StmtKind::BreakStmt => {
                if self.f.fir.cur().is_some() {
                    let j = *self.f.jumps.last().expect("break outside a loop");
                    self.emit_branch(j.brk);
                }
                Ok(())
            }
            StmtKind::ContinueStmt => {
                if self.f.fir.cur().is_some() {
                    let c = self
                        .f
                        .jumps
                        .iter()
                        .rev()
                        .find_map(|j| j.cont)
                        .expect("continue outside a loop");
                    self.emit_branch(c);
                }
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

    pub(crate) fn label_block(&mut self, d: DeclId) -> Blk {
        if let Some(&b) = self.f.labels.get(&d) {
            return b;
        }
        let b = self.f.fir.create_block();
        self.f.labels.insert(d, b);
        b
    }

    fn emit_return(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let rv = ast.stmts.return_value(s);
        if let Some(e) = rv {
            let q = ast.exprs.get(e).ty();
            let ret_ref = self.f.sig.as_ref().is_some_and(|g| self.referee(g.ret).is_some());
            if ast.types.is_void(q) || self.f.ret_slot.is_none() {
                self.emit_ignored(e)?;
            } else if ret_ref {
                let lv = self.emit_lvalue(e)?;
                let slot = self.f.ret_slot.expect("return slot");
                self.store(lv.addr.v, slot.v, slot.align, false);
            } else if self.is_aggregate(q) {
                let slot = self.f.ret_slot.expect("return slot");
                self.emit_agg(e, slot, false)?;
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
        let rb = self.f.ret_block.expect("return block");
        self.emit_branch(rb);
        Ok(())
    }

    fn emit_if(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.if_data(s);
        let kind = ast.stmts.if_kind(s);
        if matches!(kind, IfKind::Consteval | IfKind::NotConsteval) {
            let live = if kind == IfKind::Consteval { d.els } else { d.then };
            if let Some(l) = live {
                self.emit_stmt(l)?;
            }
            return Ok(());
        }
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
                    self.emit_stmt(l)?;
                }
                return Ok(());
            }
        }
        let then_b = self.f.fir.create_block();
        let end_b = self.f.fir.create_block();
        let else_b = if d.els.is_some() { self.f.fir.create_block() } else { end_b };
        self.branch_on_bool(cond, then_b, else_b)?;
        self.emit_block(then_b, false);
        if let Some(t) = d.then {
            self.emit_stmt(t)?;
        }
        self.emit_branch(end_b);
        if let Some(e) = d.els {
            self.emit_block(else_b, false);
            self.emit_stmt(e)?;
            self.emit_branch(end_b);
        }
        self.emit_block(end_b, true);
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
        let header = self.f.fir.create_block();
        self.emit_block(header, false);
        let end = self.f.fir.create_block();
        self.f.jumps.push(Jump { brk: end, cont: Some(header) });
        if let Some(v) = d.cond_var {
            self.emit_var_decl(v)?;
        }
        let cond = d.cond.expect("while condition");
        let cv = self.eval_bool(cond)?;
        let const_true = matches!(self.cint(cv), Some((_, 1, _)));
        let md = self.new_loop_md(Some(cond), d.body);
        self.f.loops.push(LoopFrame { header, md });
        let body = self.f.fir.create_block();
        if !const_true {
            self.cond_br(cv, body, end);
        }
        self.emit_block(body, false);
        if let Some(b) = d.body {
            self.emit_stmt(b)?;
        }
        self.f.jumps.pop();
        self.emit_branch(header);
        self.f.loops.pop();
        self.emit_block(end, true);
        if const_true {
            self.simplify_forwarding(header);
        }
        Ok(())
    }

    fn emit_do(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.loop_data(s);
        let end = self.f.fir.create_block();
        let cond_b = self.f.fir.create_block();
        self.f.jumps.push(Jump { brk: end, cont: Some(cond_b) });
        let body_b = self.f.fir.create_block();
        self.emit_block(body_b, false);
        if let Some(b) = d.body {
            self.emit_stmt(b)?;
        }
        self.emit_block(cond_b, false);
        let cond = d.cond.expect("do condition");
        let cv = self.eval_bool(cond)?;
        self.f.jumps.pop();
        let const_false = matches!(self.cint(cv), Some((_, 0, _)));
        let md = self.new_loop_md(Some(cond), d.body);
        self.f.loops.push(LoopFrame { header: body_b, md });
        if !const_false {
            self.cond_br(cv, body_b, end);
        }
        self.f.loops.pop();
        self.emit_block(end, false);
        if const_false {
            self.simplify_forwarding(cond_b);
        }
        Ok(())
    }

    fn emit_for(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.loop_data(s);
        let end = self.f.fir.create_block();
        if let Some(i) = d.init {
            self.emit_stmt(i)?;
        }
        let cond_b = self.f.fir.create_block();
        self.emit_block(cond_b, false);
        let md = self.new_loop_md(d.cond, d.body);
        self.f.loops.push(LoopFrame { header: cond_b, md });
        let cont = if d.inc.is_some() { self.f.fir.create_block() } else { cond_b };
        self.f.jumps.push(Jump { brk: end, cont: Some(cont) });
        if let Some(v) = d.cond_var {
            self.emit_var_decl(v)?;
        }
        if let Some(c) = d.cond {
            let body_b = self.f.fir.create_block();
            let cv = self.eval_bool(c)?;
            self.cond_br(cv, body_b, end);
            self.emit_block(body_b, false);
        }
        if let Some(b) = d.body {
            self.emit_stmt(b)?;
        }
        if let Some(inc) = d.inc {
            self.emit_block(cont, false);
            self.emit_ignored(inc)?;
        }
        self.f.jumps.pop();
        self.emit_branch(cond_b);
        self.f.loops.pop();
        self.emit_block(end, true);
        Ok(())
    }

    fn emit_switch(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let d = *ast.stmts.switch_data(s);
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
        let epilog = self.f.fir.create_block();
        let outer_cont = self.f.jumps.iter().rev().find_map(|j| j.cont);
        self.f.jumps.push(Jump { brk: epilog, cont: outer_cont });
        let saved =
            self.f.sw.replace(SwitchCtx { inst: crate::fir::Fir::last_inst(sw), cond_ty, default });
        self.emit_stmt(d.body)?;
        self.f.jumps.pop();
        if !self.f.fir.is_placed(default) {
            self.f.fir.replace_block_uses(default, epilog);
        }
        self.emit_block(epilog, true);
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
