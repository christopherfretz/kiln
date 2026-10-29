//! The constant-folding prepass (spec/codegen.md §3).
//!
//! clang's code generator asks the constant evaluator about individual
//! expressions while it emits IR (`ConstantFoldsToSimpleInteger`,
//! `tryEmitAsConstant`, `ConstantEmitter::tryEmitForInitializer`). Kiln's
//! evaluator (the Oven) needs `&mut Ast` while code generation holds the
//! AST shared, so every expression codegen may ask about is folded up
//! front, through the TU's own Oven (shared bytecode and memo), and the
//! answers are kept in [`Folds`].

use kiln_abi::Target;
use kiln_ast::consts::ConstStore;
use kiln_ast::decl::{DeclFlags, DeclKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::{RecordLayout, TypeView};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, StmtId};
use kiln_diag::Diag;
use kiln_oven::{Oven, SemaBridge, VTableLayout};
use kiln_source::fx::FxHashMap;

/// The folded values of the expressions codegen may ask about.
#[derive(Debug, Default)]
pub(crate) struct Folds {
    /// Expression → value, for every candidate that folded.
    pub expr: FxHashMap<ExprId, ConstValueId>,
}

impl Folds {
    /// The folded value of `e`, if it is a candidate that folded.
    pub fn get(&self, e: ExprId) -> Option<ConstValueId> {
        self.expr.get(&e).copied()
    }
}

/// The bridge codegen gives the Oven: no instantiation in M2, layouts
/// from `kiln_abi`, diagnostics dropped (folding is silent).
struct Bridge<'x> {
    ast: &'x mut Ast,
    abi: &'x Target,
    vt: VTableLayout,
}

impl SemaBridge for Bridge<'_> {
    fn ast(&self) -> &Ast {
        self.ast
    }

    fn ensure_body(&mut self, f: DeclId) -> Option<StmtId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }

    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout {
        kiln_abi::ensure_layout(&mut *self.ast, record, self.abi)
    }

    fn vtable(&mut self, _record: DeclId) -> &VTableLayout {
        &self.vt
    }

    fn consts(&mut self) -> &mut ConstStore {
        &mut self.ast.consts
    }

    fn diag(&mut self, _d: Diag) {}
}

/// Folds every candidate expression of the translation unit.
pub(crate) fn prepass(ast: &mut Ast, oven: &mut Oven, target: &Target) -> Folds {
    let (cands, inits) = {
        let mut c = Collect { ast, out: Vec::new(), inits: FxHashMap::default() };
        c.tu();
        (c.out, c.inits)
    };
    let mut b = Bridge { ast, abi: target, vt: VTableLayout::default() };
    let mut folds = Folds::default();
    for e in cands {
        if folds.expr.contains_key(&e) {
            continue;
        }
        // A constant initializer is manifestly constant-evaluated
        // ([expr.const]/20, clang `EvaluateAsInitializer`); anything else
        // is a trial fold (`__builtin_is_constant_evaluated()` is false).
        let v = match inits.get(&e) {
            Some(&var) => oven.fold_constant_init(&mut b, e, var),
            None => oven.try_fold(&mut b, e),
        };
        if let Some(v) = v {
            folds.expr.insert(e, v);
        }
    }
    folds
}

/// Collects the candidate expressions.
struct Collect<'a> {
    ast: &'a Ast,
    out: Vec<ExprId>,
    /// Candidates that are constant initializers of a variable (static
    /// storage, `constexpr`, or usable in constant expressions as a
    /// `const` non-volatile integral or enumeration).
    inits: FxHashMap<ExprId, DeclId>,
}

impl Collect<'_> {
    fn tu(&mut self) {
        let ast = self.ast;
        for d in ast.decls.ids() {
            let n = ast.decls.get(d);
            if n.has(DeclFlags::INVALID) {
                continue;
            }
            match n.kind() {
                DeclKind::Function => {
                    if let Some(body) = ast.decls.function(d).and_then(|f| f.body) {
                        self.stmt(body);
                    }
                }
                DeclKind::Var if !is_local(ast, d) => self.var(d, true),
                _ => {}
            }
        }
    }

    /// A variable's initializer: folded when the variable is static, an
    /// aggregate, or usable in constant expressions (clang
    /// `mightBeUsableInConstantExpressions` / `IsConstantAggregate`).
    fn var(&mut self, d: DeclId, is_static: bool) {
        let ast = self.ast;
        let Some(v) = ast.decls.var(d) else { return };
        let Some(init) = v.init else { return };
        let Some(ty) = ast.decls.get(d).ty() else { return };
        let quals = ast.types.effective_quals(ty);
        let constant_init = is_static
            || v.spec.is_constexpr()
            || (quals.is_const()
                && !quals.is_volatile()
                && (ast.types.is_integral(ty)
                    || matches!(ast.types.canonical_view(ty), TypeView::Enum(_))));
        let fold = constant_init
            || matches!(
                ast.types.canonical_view(ty),
                TypeView::ConstantArray(_)
                    | TypeView::Record(_)
                    | TypeView::LValueRef(_)
                    | TypeView::RValueRef(_)
            );
        if fold {
            self.out.push(init);
        }
        if constant_init {
            self.inits.insert(init, d);
        }
        self.expr(init);
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        let n = *ast.stmts.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => {
                for &c in ast.lists.get(ast.stmts.compound_body(s)) {
                    self.stmt(StmtId::new(c));
                }
            }
            StmtKind::DeclStmt => {
                for &d in ast.lists.get(ast.stmts.decl_stmt_decls(s)) {
                    self.local_decl(DeclId::new(d));
                }
            }
            StmtKind::ExprStmt => self.expr(ast.stmts.expr_stmt_expr(s)),
            StmtKind::IfStmt => {
                let d = *ast.stmts.if_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.local_decl(x));
                d.cond.into_iter().for_each(|x| self.cond(x));
                d.then.into_iter().for_each(|x| self.stmt(x));
                d.els.into_iter().for_each(|x| self.stmt(x));
            }
            StmtKind::SwitchStmt => {
                let d = *ast.stmts.switch_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.local_decl(x));
                self.cond(d.cond);
                self.stmt(d.body);
            }
            StmtKind::CaseStmt => {
                let c = *ast.stmts.case_data(s);
                self.out.push(c.lhs);
                self.out.extend(c.rhs);
                self.stmt(c.sub);
            }
            StmtKind::DefaultStmt => self.stmt(ast.stmts.default_sub(s)),
            StmtKind::LabelStmt | StmtKind::AttributedStmt => {
                if n.b() != 0 {
                    self.stmt(StmtId::new(n.b()));
                }
            }
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let d = *ast.stmts.loop_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.local_decl(x));
                d.cond.into_iter().for_each(|x| self.cond(x));
                d.inc.into_iter().for_each(|x| self.expr(x));
                d.body.into_iter().for_each(|x| self.stmt(x));
            }
            StmtKind::ReturnStmt => {
                if let Some(v) = ast.stmts.return_value(s) {
                    self.expr(v);
                }
            }
            _ => {}
        }
    }

    fn local_decl(&mut self, d: DeclId) {
        let ast = self.ast;
        if ast.decls.get(d).kind() == DeclKind::Var {
            let is_static = ast.decls.var(d).is_some_and(|v| {
                v.spec.storage() == VarStorage::Static || v.spec.storage() == VarStorage::Extern
            });
            self.var(d, is_static);
        }
    }

    /// A condition: the whole and its `&&`/`||`/`!`/`?:` pieces.
    fn cond(&mut self, e: ExprId) {
        self.out.push(e);
        self.expr(e);
    }

    fn expr(&mut self, e: ExprId) {
        let ast = self.ast;
        let n = ast.exprs.get(e);
        match n.kind() {
            // The operand is unevaluated: nothing in it is emitted.
            ExprKind::CXXNoexceptExpr => return,
            ExprKind::BinaryOperator
                if matches!(ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                self.out.extend(ast.exprs.operands(e));
            }
            ExprKind::ConditionalOperator => {
                self.out.push(ExprId::new(n.a()));
                self.out.extend(ast.exprs.cond_arms(e));
            }
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::LNot => {
                self.out.push(ast.exprs.operand(e));
            }
            ExprKind::UnaryExprOrTypeTraitExpr => self.out.push(e),
            ExprKind::ImplicitCastExpr
                if ast.exprs.cast_kind(e) == CastKind::LValueToRValue
                    && refers_to_const_var(ast, ast.exprs.operand(e)) =>
            {
                self.out.push(e);
            }
            _ => {}
        }
        for c in ast.exprs.children(e, &ast.lists) {
            self.expr(c);
        }
    }
}

/// Whether `e` (through parentheses) names a variable of const,
/// non-volatile type (clang `checkVarTypeForConstantEmission`).
pub(crate) fn refers_to_const_var(ast: &Ast, mut e: ExprId) -> bool {
    while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
        e = ast.exprs.operand(e);
    }
    if ast.exprs.get(e).kind() != ExprKind::DeclRefExpr {
        return false;
    }
    let d = ast.exprs.decl(e);
    if ast.decls.get(d).kind() != DeclKind::Var {
        return false;
    }
    let Some(ty) = ast.decls.get(d).ty() else { return false };
    let q = ast.types.effective_quals(ty);
    q.is_const() && !q.is_volatile()
}

/// Whether `d` is a variable declared in a function body.
pub(crate) fn is_local(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(x) = c {
        match ast.decls.get(x).kind() {
            DeclKind::Function => return true,
            DeclKind::TranslationUnit | DeclKind::Namespace => return false,
            _ => c = ast.decls.get(x).ctx(),
        }
    }
    false
}
