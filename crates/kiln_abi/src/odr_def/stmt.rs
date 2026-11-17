//! Statements in the definition hash: every [`StmtKind`], each arm
//! naming its area. A statement writes its tag, then its fixed layout:
//! optional parts are present or [`super::T_NONE`], lists are counted.

use kiln_ast::attr::AttrKind;
use kiln_ast::stmt::StmtKind as S;
use kiln_ast::{AttrId, DeclId, StmtId};

use super::decl::decl_hashed;
use super::{Area, DefHasher, Item, Site, T_LOCAL, Work};

impl DefHasher<'_> {
    /// An optional local declaration (a condition variable): its record,
    /// or [`T_NONE`].
    fn opt_decl(&mut self, d: Option<DeclId>) {
        match d {
            Some(d) => self.stack.push(Work::Decl(d)),
            None => self.stack.push(Work::OptExpr(None)),
        }
    }

    #[allow(clippy::too_many_lines, reason = "one arm per statement kind")]
    pub(super) fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        let st = &ast.stmts;
        let k = st.get(s).kind();
        match k {
            // ----- area: stmt
            S::CompoundStmt => {
                if st.compound_fp(s).mask != 0 {
                    self.partial(Area::Misc, Site::Item(Item::FpPragma));
                }
                self.stmt_tag(k);
                // Statements that hash to nothing (a contract assertion,
                // X16; a declaration that does not change the meaning) are
                // not counted: the block equals the block without them.
                let l = st.compound_body(s);
                let mark = self.stack.len();
                for c in ast.lists.iter::<StmtId>(l) {
                    if !self.invisible(c) {
                        self.stack.push(Work::Stmt(c));
                    }
                }
                self.count(self.stack.len() - mark);
            }
            S::DeclStmt => {
                self.stmt_tag(k);
                let l = st.decl_stmt_decls(s);
                let hashed = ast.lists.iter::<DeclId>(l).filter(|&d| decl_hashed(ast, d));
                let mark = self.stack.len();
                self.stack.extend(hashed.map(Work::Decl));
                self.count(self.stack.len() - mark);
            }
            S::ExprStmt => {
                self.stmt_tag(k);
                self.stack.push(Work::Expr(st.expr_stmt_expr(s)));
            }
            S::NullStmt | S::BreakStmt | S::ContinueStmt => self.stmt_tag(k),
            S::IfStmt => {
                self.stmt_tag(k);
                // `if`, `if constexpr`, `if consteval`, `if !consteval`.
                self.byte(st.if_kind(s) as u8);
                let d = *st.if_data(s);
                self.stack.push(Work::OptStmt(d.init));
                self.opt_decl(d.cond_var);
                self.stack.extend([
                    Work::OptExpr(d.cond),
                    Work::OptStmt(d.then),
                    Work::OptStmt(d.els),
                ]);
            }
            S::SwitchStmt => {
                self.stmt_tag(k);
                let d = *st.switch_data(s);
                self.stack.push(Work::OptStmt(d.init));
                self.opt_decl(d.cond_var);
                self.stack.extend([Work::Expr(d.cond), Work::Stmt(d.body)]);
            }
            S::CaseStmt => {
                self.stmt_tag(k);
                let d = *st.case_data(s);
                self.stack.extend([Work::Expr(d.lhs), Work::OptExpr(d.rhs), Work::Stmt(d.sub)]);
            }
            S::DefaultStmt => {
                self.stmt_tag(k);
                self.stack.push(Work::Stmt(st.default_sub(s)));
            }
            S::WhileStmt | S::DoStmt | S::ForStmt => {
                self.stmt_tag(k);
                let d = *st.loop_data(s);
                self.stack.push(Work::OptStmt(d.init));
                self.opt_decl(d.cond_var);
                self.stack.extend([
                    Work::OptExpr(d.cond),
                    Work::OptExpr(d.inc),
                    Work::OptStmt(d.body),
                ]);
            }
            S::CXXForRangeStmt => {
                // As written: the init-statement, the for-range-declaration
                // and the range; and the desugared pieces (`__begin`,
                // `__end`, `!=`, `++`, `*`), which are implicit but hold the
                // resolved `begin`/`end` and operator calls (O-10's one
                // implicit item). A hand-written iterator loop is another
                // statement kind: it differs.
                self.stmt_tag(k);
                let d = *st.range_for_data(s);
                self.stack.push(Work::OptStmt(d.init));
                self.stack.push(Work::Decl(d.range));
                self.opt_decl(d.begin);
                self.opt_decl(d.end);
                self.stack.extend([Work::OptExpr(d.cond), Work::OptExpr(d.inc)]);
                self.stack.push(Work::Decl(d.loop_var));
                self.stack.push(Work::Stmt(d.body));
            }
            S::ReturnStmt => {
                self.stmt_tag(k);
                self.stack.push(Work::OptExpr(st.return_value(s)));
            }
            S::LabelStmt => {
                self.stmt_tag(k);
                self.label(st.label_decl(s));
                self.stack.push(Work::Stmt(st.sub_stmt(s)));
            }
            S::GotoStmt => {
                self.stmt_tag(k);
                self.label(st.label_decl(s));
            }
            S::IndirectGotoStmt => {
                self.stmt_tag(k);
                self.stack.push(Work::Expr(st.indirect_goto_target(s)));
            }
            S::CXXTryStmt => {
                self.stmt_tag(k);
                self.stack.push(Work::Stmt(st.try_block(s)));
                let l = st.try_handlers(s);
                self.count(ast.lists.len(l));
                self.stack.extend(ast.lists.iter::<StmtId>(l).map(Work::Stmt));
            }
            S::CXXCatchStmt => {
                self.stmt_tag(k);
                self.opt_decl(st.catch_var(s));
                self.stack.push(Work::Stmt(st.catch_block(s)));
            }
            // ----- area: attrs (K-3e): `[[assume(e)]]` is on the closed
            // list; every other statement attribute is not hashed and the
            // statement is transparent.
            S::AttributedStmt => {
                let assume = ast
                    .lists
                    .get(st.stmt_attrs(s))
                    .iter()
                    .filter_map(|&r| AttrId::try_new(r))
                    .any(|a| ast.attrs.get(a).kind == AttrKind::ASSUME);
                if assume {
                    self.partial(Area::Attrs, Site::Stmt(k));
                }
                self.stack.push(Work::Stmt(st.sub_stmt(s)));
            }
            // ----- not hashed in scheme 1 (X16): not partial; dropped from
            // a block, an empty statement where a statement is required.
            S::ContractAssertStmt => self.stmt_tag(S::NullStmt),
            // ----- area: lambda (K-3d): coroutines.
            S::CoroutineBodyStmt | S::CoreturnStmt => {
                self.partial(Area::Lambda, Site::Stmt(k));
            }
            // ----- area: misc (K-3d): asm, expansion statements.
            S::GCCAsmStmt | S::CXXExpansionStmt => self.partial(Area::Misc, Site::Stmt(k)),
        }
    }

    /// Whether statement `s` of a block hashes to nothing.
    fn invisible(&self, s: StmtId) -> bool {
        let ast = self.ast;
        match ast.stmts.get(s).kind() {
            S::ContractAssertStmt => true,
            S::DeclStmt => {
                ast.lists.iter::<DeclId>(ast.stmts.decl_stmt_decls(s)).all(|d| !decl_hashed(ast, d))
            }
            _ => false,
        }
    }

    /// A label by its index (labels are locals, numbered at their first
    /// appearance, a `goto` before its label included).
    fn label(&mut self, d: DeclId) {
        self.byte(T_LOCAL);
        match self.local(d) {
            Some(i) => self.u32(i),
            None => self.declare(d),
        }
    }
}
