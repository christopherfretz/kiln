//! An imported inline `used` function named by the importer (WP-1825;
//! spec/modules.md §7 "Imported `used` functions").
//!
//! An inline function that an imported named-module unit owns is not
//! emitted by the importer until the importer names it, as any imported
//! inline function. clang 22 loads such a declaration when a name lookup
//! of the importer finds it, and hands it to codegen at once; GNU `used`
//! (an x86 `interrupt` handler is implicitly `used`) then makes it
//! `DeclMustBeEmitted`, so its `linkonce_odr` definition is emitted right
//! before the namespace-scope declaration that named it, with every
//! other `used` function of that name lookup found (its overloads), named
//! in an evaluated operand or not. kiln records them where a reference is
//! built, at the anchor `TemplateTables::ref_var_insts` gives a variable
//! instantiated at a reference.
//!
//! clang 22 and 19 also read the body of an imported function the
//! importer odr-uses (WP-1832): every declaration that body names is
//! loaded then, in the order the module file stores the body, and the
//! `used` inline functions among them are handed to codegen at that
//! point too, before the declaration that odr-used the function. kiln
//! walks the body in that order ([`Sema::note_imported_body_used`]).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{ConstexprKind, DeclKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, StmtId};

use crate::Sema;
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// Function `d`, just named by a reference: when it is an imported
    /// inline `used` function, it and the `used` overloads of its name in
    /// its context are handed to codegen before the declaration being
    /// parsed (once each).
    pub(crate) fn note_imported_used_fn(&mut self, d: DeclId) {
        if !self.ast.attrs.has_kind(AttrKind::USED) || !self.imported_used_candidate(d) {
            return;
        }
        let first = self.ast.first(d);
        if self.handed(first) {
            return;
        }
        let Some((anchor, after)) = self.ref_anchor() else { return };
        // A reference while an imported unit's own declarations are
        // replayed is that unit's, not this one's.
        if self.ast.modules.is_imported_named_unit_decl(anchor) {
            return;
        }
        let n = *self.ast.decls.get(d);
        let mut found = vec![first];
        if let Some(ctx) = n.ctx() {
            let r = self.lookup_in_dn(ctx, n.name(), LookupKind::Ordinary);
            for &o in &r.decls {
                let o = self.ast.first(o);
                if o != first && self.imported_used_candidate(o) && !found.contains(&o) {
                    found.push(o);
                }
            }
        }
        // In declaration order, as clang loads a lookup's results.
        found.sort_unstable();
        for f in found {
            if !self.handed(f) {
                self.ast.templates.add_ref_var_inst(anchor, after, f);
            }
        }
    }

    /// Function `f`, just odr-used outside a template definition and an
    /// unevaluated operand: when an imported named unit owns its
    /// definition and that body is one the module file carries, the
    /// `used` inline functions the body names are handed to codegen
    /// before the declaration being parsed, as clang hands the
    /// declarations it loads with the body (`ASTReader` reads a body on
    /// the importer's `MarkFunctionReferenced`). The body's own callees'
    /// bodies are not read (clang loads those lazily), so this is one
    /// level only; each body is walked once.
    pub(crate) fn note_imported_body_used(&mut self, f: DeclId) {
        if !self.ast.attrs.has_kind(AttrKind::USED)
            || !self.module_current_key().is_empty()
            || !self.ast.modules.is_imported_named_unit_decl(f)
        {
            return;
        }
        let first = self.ast.first(f);
        if self.imported_bodies_walked.contains(&first) {
            return;
        }
        // Where the definitions go: before the namespace-scope
        // declaration being parsed (that of the context an instantiation
        // switched from), or, during the end-of-TU drain, among the
        // definitions it hands over (clang reads the body while it
        // instantiates).
        let at = if self.inst.at_end_of_tu {
            None
        } else {
            let Some((anchor, after)) =
                self.inst_outer_anchor.map_or_else(|| self.parse_anchor(), |o| o.0)
            else {
                return;
            };
            if self.ast.modules.is_imported_named_unit_decl(anchor) {
                return;
            }
            Some((anchor, after))
        };
        let _ = self.imported_bodies_walked.insert(first);
        let Some(body) = self.imported_carried_body(first) else { return };
        let mut found: Vec<DeclId> = Vec::new();
        self.walk_body_refs(body, |s, d| {
            let d = s.ast.first(d);
            if !found.contains(&d) && s.imported_used_candidate(d) {
                found.push(d);
            }
        });
        for g in found {
            if self.handed(g) {
                continue;
            }
            match at {
                Some((anchor, after)) => self.ast.templates.add_ref_var_inst(anchor, after, g),
                None => self.ast.templates.add_end_of_tu_def(g),
            }
        }
    }

    /// The body of function `f` (a first declaration) that an importer
    /// sees in every import path: an inline, `constexpr` or `consteval`
    /// function's (the declaration image carries no other body,
    /// spec/modules.md §11.11), not a template's.
    fn imported_carried_body(&self, f: DeclId) -> Option<StmtId> {
        let ast = &self.ast;
        let def = ast.definition(f)?;
        let fd = ast.decls.function(def)?;
        if fd.templ.get().is_some() {
            return None;
        }
        let carried = ast.redecls_of(f).any(|r| {
            ast.decls.function(r).is_some_and(|x| {
                x.spec.is_inline()
                    || x.spec.implicit_inline()
                    || x.spec.constexpr_kind() != ConstexprKind::None
            })
        });
        if carried { fd.body } else { None }
    }

    /// Calls `each` with every function a `DeclRefExpr` or `MemberExpr`
    /// of statement `body` names, in the order clang's `ASTStmtReader`
    /// resolves them: a node's sub-statements last to first (the writer
    /// stores them in reverse, `ASTStmtWriter` `WriteSubStmt`), each
    /// subtree before the node itself. A declaration statement adds
    /// nothing (a local variable's initializer is read lazily), nor does
    /// a lambda's body (its call operator's) or a default argument or
    /// member initializer (not sub-statements).
    fn walk_body_refs(&self, body: StmtId, mut each: impl FnMut(&Self, DeclId)) {
        let mut stack = vec![Walk::S(body)];
        // The parts of the node being expanded, in reading order.
        let mut kids: Vec<Walk> = Vec::new();
        while let Some(n) = stack.pop() {
            match n {
                Walk::Ref(e) => {
                    let d = self.ast.exprs.decl(e);
                    if matches!(
                        self.ast.decls.get(d).kind(),
                        DeclKind::Function | DeclKind::CXXMethod
                    ) {
                        each(self, d);
                    }
                    continue;
                }
                Walk::E(e) => self.body_expr_kids(e, &mut kids),
                Walk::S(s) => self.body_stmt_kids(s, &mut kids),
            }
            stack.extend(kids.drain(..).rev());
        }
    }

    /// [`Sema::walk_body_refs`]: expression `e`'s children in reading
    /// order (`children()`, clang's `AddStmt` order, reversed), then `e`
    /// itself when it names a declaration.
    fn body_expr_kids(&self, e: ExprId, kids: &mut Vec<Walk>) {
        let exprs = &self.ast.exprs;
        let kind = exprs.get(e).kind();
        match kind {
            ExprKind::StmtExpr => kids.push(Walk::S(exprs.stmt_expr_body(e))),
            // The body is the call operator's, read with it.
            ExprKind::LambdaExpr => {}
            _ => kids.extend(exprs.children(e, &self.ast.lists).into_iter().rev().map(Walk::E)),
        }
        if matches!(kind, ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            kids.push(Walk::Ref(e));
        }
    }

    /// [`Sema::walk_body_refs`]: statement `s`'s parts in reading order:
    /// the reverse of clang's `ASTStmtWriter` `AddStmt` order, which for
    /// `if`, `switch` and `do` is not the source order.
    fn body_stmt_kids(&self, s: StmtId, kids: &mut Vec<Walk>) {
        let stmts = &self.ast.stmts;
        let lists = &self.ast.lists;
        let st = |s: Option<StmtId>, kids: &mut Vec<Walk>| kids.extend(s.map(Walk::S));
        let ex = |e: Option<ExprId>, kids: &mut Vec<Walk>| kids.extend(e.map(Walk::E));
        let at = kids.len();
        match stmts.get(s).kind() {
            StmtKind::CompoundStmt => {
                kids.extend(lists.iter::<StmtId>(stmts.compound_body(s)).map(Walk::S));
                kids[at..].reverse();
            }
            StmtKind::ExprStmt => ex(Some(stmts.expr_stmt_expr(s)), kids),
            // Written: condition, then, else, variable, init.
            StmtKind::IfStmt => {
                let i = *stmts.if_data(s);
                st(i.init, kids);
                st(i.els, kids);
                st(i.then, kids);
                ex(i.cond, kids);
            }
            // Written: condition, body, init, variable.
            StmtKind::SwitchStmt => {
                let w = *stmts.switch_data(s);
                st(w.init, kids);
                st(Some(w.body), kids);
                ex(Some(w.cond), kids);
            }
            StmtKind::CaseStmt => {
                let c = *stmts.case_data(s);
                st(Some(c.sub), kids);
                ex(c.rhs, kids);
                ex(Some(c.lhs), kids);
            }
            StmtKind::DefaultStmt => st(Some(stmts.default_sub(s)), kids),
            StmtKind::LabelStmt | StmtKind::AttributedStmt => st(Some(stmts.sub_stmt(s)), kids),
            // Written: init, condition, variable, increment, body (`for`);
            // condition, body, variable (`while`).
            StmtKind::WhileStmt | StmtKind::ForStmt => {
                let l = *stmts.loop_data(s);
                st(l.body, kids);
                ex(l.inc, kids);
                ex(l.cond, kids);
                st(l.init, kids);
            }
            // Written: condition, body.
            StmtKind::DoStmt => {
                let l = *stmts.loop_data(s);
                st(l.body, kids);
                ex(l.cond, kids);
            }
            // Written: init, the range, begin and end variables, condition,
            // increment, loop variable, body.
            StmtKind::CXXForRangeStmt => {
                let r = *stmts.range_for_data(s);
                st(Some(r.body), kids);
                ex(r.inc, kids);
                ex(r.cond, kids);
                st(r.init, kids);
            }
            StmtKind::CXXExpansionStmt => {
                let x = *stmts.expansion_data(s);
                st(x.init, kids);
                if stmts.expansion_expanded(s) {
                    kids.extend(lists.iter::<StmtId>(x.prelude).map(Walk::S));
                    kids.extend(lists.iter::<StmtId>(x.expansions).map(Walk::S));
                } else {
                    kids.extend(lists.iter::<ExprId>(x.exprs).map(Walk::E));
                    ex(x.range_init, kids);
                    kids.push(Walk::S(x.body));
                }
                kids[at..].reverse();
            }
            StmtKind::ReturnStmt => ex(stmts.return_value(s), kids),
            StmtKind::IndirectGotoStmt => ex(Some(stmts.indirect_goto_target(s)), kids),
            StmtKind::ContractAssertStmt => ex(Some(stmts.contract_cond(s)), kids),
            StmtKind::CoreturnStmt => {
                ex(stmts.coreturn_operand(s), kids);
                ex(stmts.coreturn_promise_call(s), kids);
                kids[at..].reverse();
            }
            // Written in `children()` order.
            StmtKind::CoroutineBodyStmt => {
                let c = *stmts.coroutine_body(s);
                st(Some(c.body), kids);
                st(Some(c.promise_stmt), kids);
                ex(Some(c.init_suspend), kids);
                ex(Some(c.final_suspend), kids);
                st(c.on_exception, kids);
                st(c.on_fallthrough, kids);
                ex(Some(c.allocate), kids);
                ex(Some(c.deallocate), kids);
                ex(Some(c.return_value), kids);
                st(c.return_stmt, kids);
                st(c.return_on_alloc_failure, kids);
                kids.extend(lists.iter::<StmtId>(c.param_moves).map(Walk::S));
                kids[at..].reverse();
            }
            StmtKind::GCCAsmStmt => {
                kids.extend(stmts.asm_operands(s).iter().rev().map(|op| Walk::E(op.expr)));
            }
            StmtKind::CXXTryStmt => {
                kids.push(Walk::S(stmts.try_block(s)));
                kids.extend(lists.iter::<StmtId>(stmts.try_handlers(s)).map(Walk::S));
                kids[at..].reverse();
            }
            StmtKind::CXXCatchStmt => st(Some(stmts.catch_block(s)), kids),
            // A declaration statement: a variable's initializer is read
            // lazily, with the variable's definition, not with the body.
            _ => {}
        }
    }

    /// Whether function `f` was handed over already (a short scan: the
    /// table holds the few definitions handed over at a reference; the
    /// end-of-TU list only matters during the drain).
    fn handed(&self, f: DeclId) -> bool {
        self.ast.templates.ref_var_insts().iter().any(|&(_, _, v)| v == f)
            || (self.inst.at_end_of_tu && self.ast.templates.end_of_tu_defs().contains(&f))
    }

    /// Whether `d` is a `used` inline function definition (not a
    /// template, not `consteval`) owned by an imported named-module unit.
    fn imported_used_candidate(&self, d: DeclId) -> bool {
        let ast = &self.ast;
        if !matches!(ast.decls.get(d).kind(), DeclKind::Function | DeclKind::CXXMethod)
            || !ast.modules.is_imported_named_unit_decl(d)
            || ast.inherited_attr(d, AttrKind::USED).is_none()
        {
            return false;
        }
        let Some(def) = ast.definition(d) else { return false };
        ast.decls.function(def).is_some_and(|f| f.body.is_some() && f.templ.get().is_none())
            && ast.redecls_of(d).any(|r| {
                ast.decls.function(r).is_some_and(|f| {
                    f.spec.is_inline()
                        || f.spec.implicit_inline()
                        || f.spec.constexpr_kind() == ConstexprKind::Constexpr
                })
            })
    }
}

/// A node of [`Sema::walk_body_refs`]'s walk.
enum Walk {
    S(StmtId),
    E(ExprId),
    /// A `DeclRefExpr` or `MemberExpr`, whose declaration is read after
    /// its subexpressions.
    Ref(ExprId),
}
