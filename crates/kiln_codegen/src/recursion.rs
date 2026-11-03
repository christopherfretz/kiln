//! clang `CodeGenModule::isTriviallyRecursive` (spec/codegen.md
//! "Explicit instantiation declarations"): an `available_externally`
//! body is not emitted for a function that calls itself through its
//! asm label, directly or as the library function a `__builtin_` names
//! (glibc's `btowc` pattern: such a body is not equivalent to the real
//! definition elsewhere, PR9614).

use kiln_ast::decl::{DeclKind, builtin};
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, StmtId};

use crate::module::Cg;

impl Cg<'_> {
    /// Whether function `first`'s definition is trivially recursive. Its
    /// name is its asm label (every function reaching here is mangled,
    /// so one without a label is never recursive); the walk is clang's
    /// `FunctionIsDirectlyRecursive`: every statement and expression of
    /// the body, except that a call's operands are not visited.
    pub(crate) fn is_trivially_recursive(&self, first: DeclId) -> bool {
        let ast = self.ast;
        let Some(name) = kiln_abi::mangle::asm_label(ast, first) else {
            return false;
        };
        let Some(body) =
            ast.definition(first).and_then(|d| ast.decls.function(d)).and_then(|f| f.body)
        else {
            return false;
        };
        let mut stmts: Vec<StmtId> = vec![body];
        let mut exprs: Vec<ExprId> = Vec::new();
        loop {
            if let Some(e) = exprs.pop() {
                match ast.exprs.get(e).kind() {
                    ExprKind::CallExpr
                    | ExprKind::CXXMemberCallExpr
                    | ExprKind::CXXOperatorCallExpr => {
                        if self.calls_label(e, &name) {
                            return true;
                        }
                    }
                    // A call too in clang; its operator is never labelled
                    // as a function the walk could find.
                    ExprKind::UserDefinedLiteral => {}
                    ExprKind::StmtExpr => stmts.push(ast.exprs.stmt_expr_body(e)),
                    _ => exprs.extend(ast.exprs.children(e, &ast.lists)),
                }
                continue;
            }
            let Some(s) = stmts.pop() else { return false };
            self.stmt_operands(s, &mut stmts, &mut exprs);
        }
    }

    /// The child statements and expressions of `s` (clang
    /// `Stmt::children`; a declaration statement's are its variables'
    /// initializers).
    fn stmt_operands(&self, s: StmtId, stmts: &mut Vec<StmtId>, exprs: &mut Vec<ExprId>) {
        let ast = self.ast;
        let st = &ast.stmts;
        let var_init = |d: DeclId, exprs: &mut Vec<ExprId>| {
            exprs.extend(ast.decls.var(d).and_then(|v| v.init));
        };
        match st.get(s).kind() {
            StmtKind::CompoundStmt => stmts.extend(ast.lists.iter::<StmtId>(st.compound_body(s))),
            StmtKind::DeclStmt => {
                for d in ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)) {
                    var_init(d, exprs);
                }
            }
            StmtKind::ExprStmt => exprs.push(st.expr_stmt_expr(s)),
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                stmts.extend(d.init.into_iter().chain(d.then).chain(d.els));
                if let Some(v) = d.cond_var {
                    var_init(v, exprs);
                }
                exprs.extend(d.cond);
            }
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                stmts.extend(d.init.into_iter().chain(Some(d.body)));
                if let Some(v) = d.cond_var {
                    var_init(v, exprs);
                }
                exprs.push(d.cond);
            }
            StmtKind::CaseStmt => {
                let d = *st.case_data(s);
                exprs.extend(Some(d.lhs).into_iter().chain(d.rhs));
                stmts.push(d.sub);
            }
            StmtKind::DefaultStmt => stmts.push(st.default_sub(s)),
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                let d = *st.loop_data(s);
                stmts.extend(d.init.into_iter().chain(d.body));
                if let Some(v) = d.cond_var {
                    var_init(v, exprs);
                }
                exprs.extend(d.cond.into_iter().chain(d.inc));
            }
            StmtKind::CXXForRangeStmt => {
                let d = *st.range_for_data(s);
                stmts.extend(d.init.into_iter().chain(Some(d.body)));
                for v in [Some(d.range), d.begin, d.end, Some(d.loop_var)].into_iter().flatten() {
                    var_init(v, exprs);
                }
                exprs.extend(d.cond.into_iter().chain(d.inc));
            }
            StmtKind::LabelStmt | StmtKind::AttributedStmt => {
                stmts.push(StmtId::new(st.get(s).b()));
            }
            StmtKind::ReturnStmt => exprs.extend(st.return_value(s)),
            StmtKind::CoreturnStmt => exprs.extend(st.coreturn_operand(s)),
            StmtKind::CXXTryStmt => {
                stmts.push(st.try_block(s));
                for h in ast.lists.iter::<StmtId>(st.try_handlers(s)) {
                    stmts.push(st.catch_block(h));
                }
            }
            _ => {}
        }
    }

    /// clang `FunctionIsDirectlyRecursive::VisitCallExpr`: the call's
    /// direct callee carries asm label `name`, or is a library builtin
    /// `__builtin_<name>` (one whose library function kiln recognizes,
    /// clang `Builtin::Context::isLibFunction`).
    fn calls_label(&self, call: ExprId, name: &str) -> bool {
        let ast = self.ast;
        let mut c = ast.exprs.callee(call);
        let f = loop {
            match ast.exprs.get(c).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => c = ast.exprs.operand(c),
                ExprKind::SubstNonTypeTemplateParmExpr => {
                    match ast.exprs.children(c, &ast.lists).last() {
                        Some(&r) => c = r,
                        None => return false,
                    }
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => break ast.exprs.decl(c),
                _ => return false,
            }
        };
        if !matches!(ast.decls.get(f).kind(), DeclKind::Function | DeclKind::CXXMethod) {
            return false;
        }
        if kiln_abi::mangle::asm_label(ast, f).is_some_and(|l| l == name) {
            return true;
        }
        ast.decls
            .builtin_id(f)
            .and_then(builtin::info)
            .and_then(|i| i.name.strip_prefix("__builtin_"))
            .is_some_and(|lib| lib == name && builtin::lib_by_name(lib).is_some())
    }
}
