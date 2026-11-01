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
use kiln_ast::types::{RecordLayout, TypeFlags, TypeView};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, StmtId};
use kiln_diag::Diag;
use kiln_oven::{Oven, SemaBridge, Subobject, SubobjectId};
use kiln_source::fx::{FxHashMap, FxHashSet};

/// The folded values of the expressions codegen may ask about.
#[derive(Debug, Default)]
pub(crate) struct Folds {
    /// Expression → value, for every candidate that folded.
    pub expr: FxHashMap<ExprId, ConstValueId>,
    /// The values of the immediate invocations inside default arguments,
    /// per use: (invocation, the enclosing default argument use whose
    /// source-location values apply, as [`crate::module`]'s `srcloc`).
    pub imm: FxHashMap<(ExprId, Option<ExprId>), ConstValueId>,
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
}

impl SemaBridge for Bridge<'_> {
    fn ast(&self) -> &Ast {
        self.ast
    }

    fn ensure_body(&mut self, f: DeclId, _oven: &mut Oven) -> Option<StmtId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }

    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout {
        kiln_abi::ensure_layout(&mut *self.ast, record, self.abi)
    }

    // A vector's size and alignment: the Oven lays out its lanes inside
    // them (WP-173).
    fn opaque_size_align(&mut self, ty: QualType) -> (u64, u32) {
        let l = kiln_abi::type_size_align(self.ast, ty, self.abi);
        (l.size, u32::try_from(l.align).unwrap_or(1))
    }

    fn final_overrider(
        &mut self,
        most_derived: DeclId,
        at: SubobjectId,
        method: DeclId,
    ) -> Option<(DeclId, SubobjectId)> {
        let first = self.ast.first(most_derived);
        self.ast.virtuals.get(first)?.final_overrider(at, method)
    }

    fn subobjects(&mut self, record: DeclId) -> &[Subobject] {
        let first = self.ast.first(record);
        self.ast.virtuals.get(first).map_or(&[], |v| &v.subobjects)
    }

    fn consts(&mut self) -> &mut ConstStore {
        &mut self.ast.consts
    }

    fn diag(&mut self, _d: Diag) {}

    fn set_temporary_value(&mut self, t: DeclId, v: ConstValueId) {
        // The `_ZGR` global of a constant-initialized reference holds the
        // value (write-once, as sema's bridge).
        let _ = self.ast.decls.set_lifetime_extended_value(t, v);
    }
}

/// Folds every candidate expression of the translation unit.
pub(crate) fn prepass(ast: &mut Ast, oven: &mut Oven, target: &Target) -> Folds {
    let (cands, inits, imm) = {
        let mut c = Collect {
            ast,
            out: Vec::new(),
            inits: FxHashMap::default(),
            imm: Vec::new(),
            defaults: FxHashSet::default(),
        };
        c.tu();
        (c.out, c.inits, c.imm)
    };
    let mut b = Bridge { ast, abi: target };
    let mut folds = Folds::default();
    for e in cands {
        if folds.expr.contains_key(&e) {
            continue;
        }
        // A constant initializer is manifestly constant-evaluated
        // ([expr.const]/20, clang `EvaluateAsInitializer`); anything else
        // is a trial fold (`__builtin_is_constant_evaluated()` is false),
        // lax as clang's `EvaluateAsRValue`: a `const double` or
        // `const _Atomic(int)` variable reads as its value.
        let v = match inits.get(&e) {
            Some(&var) => oven.fold_constant_init(&mut b, e, var),
            None => oven.fold_rvalue(&mut b, e),
        };
        if let Some(v) = v {
            folds.expr.insert(e, v);
        }
    }
    for key @ (e, at) in imm {
        if folds.imm.contains_key(&key) {
            continue;
        }
        let op = b.ast.exprs.operand(e);
        if let Some(v) = oven.fold_immediate_in(&mut b, op, at) {
            folds.imm.insert(key, v);
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
    /// Immediate invocations inside default arguments, with the use
    /// they are evaluated in ([`Folds::imm`]).
    imm: Vec<(ExprId, Option<ExprId>)>,
    /// Parameters whose default argument was already walked: a
    /// `CXXDefaultArgExpr` has no children, its expression is shared.
    defaults: FxHashSet<DeclId>,
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
                DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion => {
                    if let Some(body) = ast.decls.function(d).and_then(|f| f.body) {
                        if n.kind() == DeclKind::CXXConstructor {
                            for ci in ast.ctor_inits(d) {
                                self.expr(ci.init);
                            }
                        }
                        self.stmt(body);
                    }
                }
                DeclKind::Var if !is_local(ast, d) => self.var(d, true),
                // A default member initializer's immediate invocations
                // are left unevaluated by sema, like a default argument's.
                DeclKind::Field => {
                    if let Some(init) = ast.decls.field(d).and_then(|f| f.init) {
                        self.immediates(init, None, 0);
                    }
                }
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
        // A reference is usable in constant expressions when its
        // initializer is constant ([expr.const]/4): its named uses read
        // through the value (`Cg::constant_ref_lvalue`, WP-231).
        let constant_init = is_static
            || v.spec.is_constexpr()
            || ast.types.is_reference(ty)
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
                if n.kind() == StmtKind::AttributedStmt {
                    for a in ast.lists.iter::<kiln_ast::AttrId>(ast.stmts.stmt_attrs(s)) {
                        let node = *ast.attrs.get(a);
                        if node.kind == kiln_ast::attr::AttrKind::ASSUME
                            && let Some(e) = <ExprId as kiln_ast::ids::RawId>::from_raw(node.a)
                        {
                            self.expr(e);
                        }
                    }
                }
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
            StmtKind::CXXForRangeStmt => {
                let d = *ast.stmts.range_for_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                self.local_decl(d.range);
                d.begin.into_iter().for_each(|x| self.local_decl(x));
                d.end.into_iter().for_each(|x| self.local_decl(x));
                d.cond.into_iter().for_each(|x| self.cond(x));
                d.inc.into_iter().for_each(|x| self.expr(x));
                self.local_decl(d.loop_var);
                self.stmt(d.body);
            }
            StmtKind::ReturnStmt => {
                if let Some(v) = ast.stmts.return_value(s) {
                    self.expr(v);
                }
            }
            StmtKind::GCCAsmStmt => {
                for op in ast.stmts.asm_operands(s) {
                    self.expr(op.expr);
                }
            }
            // The try block and its handlers are emitted (a constant read
            // there must fold as anywhere else, or it loads from a static
            // member that has no definition).
            StmtKind::CXXTryStmt => {
                self.stmt(ast.stmts.try_block(s));
                for h in ast.lists.iter::<StmtId>(ast.stmts.try_handlers(s)) {
                    self.stmt(h);
                }
            }
            StmtKind::CXXCatchStmt => {
                ast.stmts.catch_var(s).into_iter().for_each(|v| self.local_decl(v));
                self.stmt(ast.stmts.catch_block(s));
            }
            _ => {}
        }
    }

    fn local_decl(&mut self, d: DeclId) {
        let ast = self.ast;
        // The variable length array bounds `EmitVariablyModifiedType`
        // evaluates (WP-183).
        match ast.decls.get(d).kind() {
            DeclKind::Var => {
                if let Some(t) = ast.decls.get(d).ty() {
                    self.vla_bounds(t);
                }
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                if let Some(t) = ast.decls.typedef(d) {
                    self.vla_bounds(t.underlying);
                }
            }
            _ => {}
        }
        if ast.decls.get(d).kind() == DeclKind::Var {
            let is_static = ast.decls.var(d).is_some_and(|v| {
                v.spec.storage() == VarStorage::Static || v.spec.storage() == VarStorage::Extern
            });
            self.var(d, is_static);
        }
    }

    /// The bounds of the variable length arrays in `q` (through arrays,
    /// pointers and references, like codegen's `emit_vla_bounds`).
    fn vla_bounds(&mut self, mut q: QualType) {
        let ast = self.ast;
        loop {
            if !ast.types.qflags(q).contains(TypeFlags::VARIABLY_MODIFIED) {
                return;
            }
            match ast.types.canonical_view(q) {
                TypeView::VariableArray { elem, size } => {
                    self.expr(size);
                    q = elem;
                }
                TypeView::ConstantArray(a) => q = a.elem,
                TypeView::IncompleteArray(e)
                | TypeView::Pointer(e)
                | TypeView::LValueRef(e)
                | TypeView::RValueRef(e) => q = e,
                _ => return,
            }
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
            ExprKind::CXXNoexceptExpr
            | ExprKind::TypeTraitExpr
            | ExprKind::ArrayTypeTraitExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::ConceptSpecializationExpr
            | ExprKind::RequiresExpr => return,
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
            ExprKind::UnaryExprOrTypeTraitExpr => {
                self.out.push(e);
                if let Some(t) = ast.exprs.trait_type_arg(e) {
                    self.vla_bounds(t);
                }
            }
            // clang `ScalarExprEmitter::VisitMemberExpr` tries
            // `EvaluateAsInt` on an integral member access.
            ExprKind::MemberExpr
                if ast.types.is_integral(n.ty())
                    || matches!(ast.types.canonical_view(n.ty()), TypeView::Enum(_)) =>
            {
                self.out.push(e);
            }
            // clang `EmitBuiltinExpr` first tries the call as a constant.
            ExprKind::CallExpr if crate::builtins::is_fold_candidate(ast, e) => {
                self.out.push(e);
                self.out.extend(crate::builtins::folded_arg(ast, e));
            }
            // clang `EmitCXXNewAllocSize` tries the array bound as a constant.
            ExprKind::CXXNewExpr => self.out.extend(ast.exprs.new_extra(e).array_size),
            ExprKind::StmtExpr => self.stmt(ast.exprs.stmt_expr_body(e)),
            // The default argument is emitted at each call that uses it;
            // its immediate invocations are evaluated per use.
            ExprKind::CXXDefaultArgExpr => {
                let p = ast.exprs.default_arg_param(e);
                if self.defaults.insert(p)
                    && let Some(d) = ast.decls.param(p).and_then(|x| x.default_arg)
                {
                    self.expr(d);
                }
                self.default_arg(e, None, 0);
            }
            // A default member initializer use with its own source-location
            // values: its immediate invocations are evaluated per use.
            ExprKind::CXXDefaultInitExpr if ast.exprs.source_loc_values(e).is_some() => {
                self.default_init(e, None, 0);
            }
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

impl Collect<'_> {
    /// The immediate invocations of the default argument use `u` (nested
    /// in the use `at`, if any): sema leaves them unevaluated in the
    /// parameter's default argument (spec/sema.md §5.10), so the prepass
    /// evaluates them per use, with that use's source-location values
    /// (clang `BuildCXXDefaultArgExpr`'s rewritten argument).
    fn default_arg(&mut self, u: ExprId, at: Option<ExprId>, depth: u32) {
        let ast = self.ast;
        let p = ast.exprs.default_arg_param(u);
        let Some(d) = ast.decls.param(p).and_then(|x| x.default_arg) else { return };
        if depth > 32 {
            return;
        }
        let at = at.or_else(|| ast.exprs.source_loc_values(u).is_some().then_some(u));
        self.immediates(d, at, depth);
    }

    /// As [`Collect::default_arg`], for the default member initializer
    /// use `u`.
    fn default_init(&mut self, u: ExprId, at: Option<ExprId>, depth: u32) {
        let ast = self.ast;
        let f = ast.exprs.default_init_field(u);
        let Some(d) = ast.decls.field(f).and_then(|x| x.init) else { return };
        if depth > 32 {
            return;
        }
        let at = at.or_else(|| ast.exprs.source_loc_values(u).is_some().then_some(u));
        self.immediates(d, at, depth);
    }

    /// The unevaluated immediate invocations in `root`, a default argument
    /// or default member initializer (evaluated in the use `at`).
    fn immediates(&mut self, root: ExprId, at: Option<ExprId>, depth: u32) {
        let ast = self.ast;
        let mut stack = vec![root];
        while let Some(x) = stack.pop() {
            match ast.exprs.get(x).kind() {
                ExprKind::ConstantExpr if ast.exprs.constant_value(x).is_none() => {
                    self.imm.push((x, at));
                }
                ExprKind::CXXDefaultArgExpr => self.default_arg(x, at, depth + 1),
                ExprKind::CXXDefaultInitExpr if at.is_some() => {
                    self.default_init(x, at, depth + 1);
                }
                ExprKind::CXXNoexceptExpr
                | ExprKind::TypeTraitExpr
                | ExprKind::ArrayTypeTraitExpr
                | ExprKind::ExpressionTraitExpr => {}
                _ => stack.extend(ast.exprs.children(x, &ast.lists)),
            }
        }
    }
}

/// Whether `e` (through parentheses and no-op casts) names a variable of const,
/// non-volatile type (clang `checkVarTypeForConstantEmission`).
pub(crate) fn refers_to_const_var(ast: &Ast, mut e: ExprId) -> bool {
    // Through parentheses and no-op casts (the xvalue of an implicitly
    // movable `return x;`): clang's `VisitCastExpr` visits a `CK_NoOp`
    // operand, reaching `VisitDeclRefExpr`'s `tryEmitAsConstant`.
    loop {
        match ast.exprs.get(e).kind() {
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            ExprKind::ImplicitCastExpr if ast.exprs.cast_kind(e) == CastKind::NoOp => {
                e = ast.exprs.operand(e);
            }
            _ => break,
        }
    }
    if ast.exprs.get(e).kind() != ExprKind::DeclRefExpr {
        return false;
    }
    let d = ast.exprs.decl(e);
    if ast.decls.get(d).kind() != DeclKind::Var {
        return false;
    }
    let Some(mut ty) = ast.decls.get(d).ty() else { return false };
    // A reference to a const object (clang `CEK_AsValueOrReference`): the
    // read folds to the referee's value when that is constant (WP-231).
    if let Some(p) = ast.types.pointee(ty).filter(|_| ast.types.is_reference(ty)) {
        ty = p;
    }
    let q = ast.types.effective_quals(ty);
    q.is_const() && !q.is_volatile()
}

/// Whether `d` is a variable declared in a function body.
pub(crate) fn is_local(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(x) = c {
        match ast.decls.get(x).kind() {
            k if k.is_function() => return true,
            DeclKind::TranslationUnit | DeclKind::Namespace => return false,
            _ => c = ast.decls.get(x).ctx(),
        }
    }
    false
}
