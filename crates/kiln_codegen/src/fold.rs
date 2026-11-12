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
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, UnaryOp};
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
    /// The variables (first declarations) whose type has a non-trivial
    /// destructor but which have constant destruction ([expr.const]/7):
    /// they need no destructor call (clang `VarDecl::needsDestruction`
    /// is `DK_none` once `evaluateDestruction` succeeded).
    pub const_destroyed: FxHashSet<DeclId>,
    /// WP-808: the object sizes of the `pass_object_size(t)` arguments
    /// that fold (clang `Expr::tryEvaluateObjectSize`), by (argument,
    /// `t`).
    pub obj_size: FxHashMap<(ExprId, u32), u64>,
}

impl Folds {
    /// The folded value of `e`, if it is a candidate that folded.
    pub fn get(&self, e: ExprId) -> Option<ConstValueId> {
        self.expr.get(&e).copied()
    }
}

/// The bridge codegen gives the Oven: no instantiation in M2, layouts
/// from `kiln_abi`, diagnostics dropped (folding is silent), the language
/// mode sema recorded in the Oven ([`Oven::cxx_year`]).
struct Bridge<'x> {
    ast: &'x mut Ast,
    abi: &'x Target,
    cxx_year: u32,
}

impl SemaBridge for Bridge<'_> {
    fn ast(&self) -> &Ast {
        self.ast
    }

    fn cxx_year(&self) -> u32 {
        self.cxx_year
    }

    fn ensure_body(&mut self, f: DeclId, _oven: &mut Oven) -> Option<StmtId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }

    fn ensure_layout(&mut self, record: DeclId) -> Option<&RecordLayout> {
        kiln_abi::ensure_layout(&mut *self.ast, record, self.abi)
    }

    // A vector's size and alignment: the Oven lays out its lanes inside
    // them (WP-173).
    fn opaque_size_align(&mut self, ty: QualType) -> (u64, u32) {
        let l = kiln_abi::type_size_align(self.ast, ty, self.abi);
        (l.size, u32::try_from(l.align).unwrap_or(1))
    }

    fn decl_align(&mut self, d: DeclId) -> u64 {
        kiln_abi::layout::decl_align(self.ast, d, self.abi, false)
    }

    fn alignof_decl(&mut self, d: DeclId) -> u64 {
        kiln_abi::layout::decl_align(self.ast, d, self.abi, true)
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

    fn lvalue_ref_to(&mut self, t: QualType) -> Option<QualType> {
        Some(self.ast.types.lvalue_ref_to(t))
    }
}

/// Folds every candidate expression of the translation unit.
pub(crate) fn prepass(ast: &mut Ast, oven: &mut Oven, target: &Target) -> Folds {
    let (cands, inits, c_static, imm, statics, dtors, ps_args, arr_lists) = {
        let mut c = Collect {
            ast,
            compat: oven.clang_compat(),
            out: Vec::new(),
            inits: FxHashMap::default(),
            c_static: FxHashSet::default(),
            imm: Vec::new(),
            defaults: FxHashSet::default(),
            statics: Vec::new(),
            dtors: FxHashMap::default(),
            ps_args: Vec::new(),
            arr_lists: Vec::new(),
            cover: None,
        };
        c.tu();
        (c.out, c.inits, c.c_static, c.imm, c.statics, c.dtors, c.ps_args, c.arr_lists)
    };
    let mut b = Bridge { ast, abi: target, cxx_year: oven.cxx_year() };
    let mut folds = Folds::default();
    let mut refolded = FxHashSet::default();
    let mut tried = FxHashSet::default();
    // clang's `ConstantEmitter` folds in modes that evaluate past
    // floating arithmetic yielding a NaN or dividing by zero
    // (`keepEvaluatingAfterUndefinedBehavior`).
    oven.set_fold_float_ub(true);
    // clang's folds outside a constant context let allocations leak
    // (`CheckMemoryLeaks` is a `CCEDiag`): `(bool)new int(4)` is `true`
    // (WP-1108).
    oven.set_fold_leaks(true);
    for e in cands {
        if !tried.insert(e) {
            continue;
        }
        // A constant initializer is manifestly constant-evaluated
        // ([expr.const]/20, clang `EvaluateAsInitializer`); anything else
        // is a trial fold (`__builtin_is_constant_evaluated()` is false),
        // lax as clang's `EvaluateAsRValue`: a `const double` or
        // `const _Atomic(int)` variable reads as its value.
        // A C static's initializer is evaluated in a constant context
        // (clang: every C variable `hasConstantInitialization`).
        oven.set_fold_c_static(c_static.contains(&e));
        let v = match inits.get(&e) {
            // A `constexpr` variable sema already evaluated keeps that value
            // (clang `VarDecl::evaluateValue` is cached and
            // `tryEmitForInitializer` reads it): its initializer is
            // manifestly constant-evaluated either way, and only sema's
            // bridge runs the reflection intrinsics (`constexpr bool b =
            // type_of(R) == ^^int;`, spec/reflection.md §8.3; `static
            // constexpr auto s = std::define_static_array(r);`, WP-479).
            Some(&var) if let Some(v) = sema_value(b.ast, var) => Some(v),
            Some(&var) => match oven.fold_constant_init(&mut b, e, var) {
                // clang `ConstantEmitter::tryEmitPrivateForVarInit`: an
                // initializer that is not constant is folded again outside
                // a constant context (`__builtin_is_constant_evaluated()`
                // is false, a cast only a fold performs continues), and a
                // value makes it a static initializer all the same. The
                // variable is not constant-initialized, so neither its
                // uses nor its destruction see the value. Scalars,
                // vectors and initializer lists (spec/codegen.md "Static
                // initialization").
                None if is_scalar(b.ast, var) || is_init_list(b.ast, e) => {
                    // An address cast to an integer stays one
                    // (`static intptr_t i = (intptr_t)&g;`, C23 6.6p9).
                    oven.set_fold_addr_int(true);
                    let v = oven.fold_rvalue(&mut b, e);
                    oven.set_fold_addr_int(false);
                    if v.is_some() {
                        refolded.insert(e);
                    }
                    v
                }
                v => v,
            },
            None => oven.fold_rvalue(&mut b, e),
        };
        oven.set_fold_c_static(false);
        // A value holding the address of an automatic variable (a
        // `constexpr` reference to a local, P2686R5) has no constant
        // form: like clang's `ConstantEmitter`, which cannot emit it,
        // codegen evaluates the expression at run time.
        if let Some(v) = v.filter(|&v| kiln_oven::automatic_referent(b.ast, v).is_none()) {
            folds.expr.insert(e, v);
        }
    }
    array_lists(&mut b, oven, &mut folds, &tried, arr_lists);
    for (e, init) in statics {
        if let Some(v) = folds.get(init).filter(|_| !refolded.contains(&init)) {
            folds.expr.entry(e).or_insert(v);
        }
    }
    for key @ (e, at) in imm {
        if folds.imm.contains_key(&key) {
            continue;
        }
        // Sema's value at a use without source-location values: the same
        // evaluation, not run again (WP-1129).
        if at.is_none()
            && let Some(v) = b.ast.exprs.default_immediate_value(e)
        {
            folds.imm.insert(key, v);
            continue;
        }
        let op = b.ast.exprs.operand(e);
        if let Some(v) = oven.fold_immediate_in(&mut b, op, at) {
            folds.imm.insert(key, v);
        }
    }
    // clang `evaluateOrEmitBuiltinObjectSize` of each `pass_object_size`
    // argument (WP-808).
    for key @ (a, t) in ps_args {
        if !folds.obj_size.contains_key(&key)
            && let Some(n) = oven.fold_object_size(&mut b, a, t)
        {
            folds.obj_size.insert(key, n);
        }
    }
    // clang `Sema::FinalizeVarWithDestructor`: `evaluateValue`, then
    // `evaluateDestruction` of that value (a `const` variable's object;
    // any other's is outside its lifetime), manifestly constant-evaluated
    // when the variable is constant-initialized.
    oven.set_fold_float_ub(false);
    oven.set_fold_leaks(false);
    let mut dtors: Vec<_> = dtors.into_iter().collect();
    dtors.sort_unstable_by_key(|&(first, _)| first);
    for (first, (d, init, is_static)) in dtors {
        let value = init.filter(|e| !refolded.contains(e)).and_then(|e| folds.get(e));
        let const_context = is_static && (init.is_none() || value.is_some());
        if oven.has_constant_destruction(&mut b, d, value, const_context) {
            folds.const_destroyed.insert(first);
        }
    }
    folds
}

/// clang `AggExprEmitter::EmitArrayInit` (WP-1008): an array
/// initializer list of trivially copyable elements whose explicit
/// elements take more than 16 bytes is copied from a private `constinit`
/// global when `ConstantEmitter::tryEmitForInitializer` emits it. That
/// emitter folds each element it cannot spell directly with
/// `EvaluateAsRValue` outside a constant context, so the list folds as a
/// trial rvalue here, once. A list whose covering initializer folded is
/// never emitted (its value is), and an expression the main pass already
/// tried keeps that answer.
fn array_lists(
    b: &mut Bridge<'_>,
    oven: &mut Oven,
    folds: &mut Folds,
    tried: &FxHashSet<ExprId>,
    lists: Vec<(ExprId, Option<ExprId>)>,
) {
    let mut covered = FxHashSet::default();
    for (e, cover) in lists {
        if cover.is_some_and(|c| covered.contains(&c) || folds.expr.contains_key(&c)) {
            covered.insert(e);
            continue;
        }
        if tried.contains(&e) || folds.expr.contains_key(&e) {
            continue;
        }
        let ty = b.ast.exprs.get(e).ty();
        let Some(elem) = b.ast.types.element(ty) else { continue };
        let n = match b.ast.exprs.get(e).kind() {
            ExprKind::CXXParenListInitExpr => {
                b.ast.lists.get(b.ast.exprs.paren_list_inits(e)).len()
            }
            _ => b.ast.lists.get(b.ast.exprs.init_list_inits(e)).len(),
        };
        let size = kiln_abi::type_size_align(b.ast, elem, b.abi).size;
        if (n as u64).saturating_mul(size) <= 16 {
            continue;
        }
        if let Some(v) =
            oven.fold_rvalue(b, e).filter(|&v| kiln_oven::automatic_referent(b.ast, v).is_none())
        {
            folds.expr.insert(e, v);
        }
    }
}

/// The value sema computed for `constexpr` variable `d`'s initializer.
/// Only a `constexpr` one: sema records values only there today
/// (`check_constexpr_init`), and a value computed for any other variable
/// is the initialization's value only if the variable really is
/// constant-initialized (`int y = std::is_constant_evaluated();` is 0 at
/// run time), so the folder decides that itself.
/// GNU: the initializer of the flexible array member that the list
/// `init` of a static object gives elements (WP-1286). The Oven builds
/// the object without them; they are folded on their own and
/// `Cg::record_elems` appends them past the object.
pub(crate) fn flexible_elements_init(ast: &Ast, init: ExprId) -> Option<ExprId> {
    if ast.exprs.get(init).kind() != ExprKind::InitListExpr {
        return None;
    }
    let r = ast.types.as_record(ast.exprs.get(init).ty())?;
    let last = ast
        .decls
        .members(ast.definition(r)?)
        .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
        .last()?;
    let lt = ast.decls.get(last).ty()?;
    let x = *crate::expr::agg_init_parts(ast, init).0.last()?;
    let sized = matches!(
        ast.types.canonical_view(ast.exprs.get(x).ty()),
        TypeView::ConstantArray(c) if c.size > 0
    );
    (matches!(ast.types.canonical_view(lt), TypeView::IncompleteArray(_)) && sized).then_some(x)
}

fn sema_value(ast: &Ast, d: DeclId) -> Option<ConstValueId> {
    let v = ast.decls.var(d)?;
    match v.eval.state() {
        kiln_ast::decl::VarEvalState::Value(c) if v.spec.is_constexpr() => Some(c),
        _ => None,
    }
}

/// Whether variable `d` has a scalar type (arithmetic, enumeration,
/// pointer or pointer to member) or a vector type: clang
/// `ConstantEmitter::tryEmitPrivate` folds either with `EvaluateAsRValue`
/// (`i4 gv = cv;` with `const i4 cv = {..}` is a static initializer,
/// WP-453); aggregates go through `ConstExprEmitter` instead.
fn is_scalar(ast: &Ast, d: DeclId) -> bool {
    ast.decls.get(d).ty().is_some_and(|t| {
        matches!(
            ast.types.canonical_view(t),
            TypeView::Builtin(_)
                | TypeView::BitInt { .. }
                | TypeView::Pointer(_)
                | TypeView::MemberPointer { .. }
                | TypeView::Enum(_)
                | TypeView::Vector { .. }
        )
    })
}

/// Whether a member access of type `q` is visited by clang's
/// `ScalarExprEmitter::VisitMemberExpr` (vectors are left to the load).
fn is_scalar_member(ast: &Ast, q: QualType) -> bool {
    matches!(
        ast.types.canonical_view(q),
        TypeView::Builtin(_)
            | TypeView::BitInt { .. }
            | TypeView::Pointer(_)
            | TypeView::MemberPointer { .. }
            | TypeView::Enum(_)
    )
}

/// Whether initializer `e` is an initializer list: clang's
/// `ConstExprEmitter` emits each element with `tryEmitPrivateForMemory`,
/// which folds an element it cannot emit directly with `EvaluateAsRValue`
/// (outside a constant context once constant initialization failed), so
/// a list whose elements all fold is a static initializer (`H h =
/// {(void (*)())&f, 1};`, WP-492). A constructor call is not: the
/// emitter takes only a trivial one, and a list with temporaries
/// (`ExprWithCleanups`) is left dynamic.
fn is_init_list(ast: &Ast, e: ExprId) -> bool {
    ast.exprs.get(e).kind() == ExprKind::InitListExpr
}

/// Collects the candidate expressions.
struct Collect<'a> {
    ast: &'a Ast,
    /// The `-fclang-compat` level ([`crate::builtins::is_fold_candidate`]).
    compat: u32,
    out: Vec<ExprId>,
    /// Candidates that are constant initializers of a variable (static
    /// storage, `constexpr`, or usable in constant expressions as a
    /// `const` non-volatile integral or enumeration).
    inits: FxHashMap<ExprId, DeclId>,
    /// The initializers of a C translation unit's static-storage
    /// variables, folded in a constant context
    /// ([`Oven::set_fold_c_static`]).
    c_static: FxHashSet<ExprId>,
    /// Immediate invocations inside default arguments, with the use
    /// they are evaluated in ([`Folds::imm`]).
    imm: Vec<(ExprId, Option<ExprId>)>,
    /// Parameters whose default argument was already walked: a
    /// `CXXDefaultArgExpr` has no children, its expression is shared.
    defaults: FxHashSet<DeclId>,
    /// Member accesses naming a `const` static data member, with that
    /// member's initializer: the access folds to its value.
    statics: Vec<(ExprId, ExprId)>,
    /// Variables whose type has a non-trivial destructor, by first
    /// declaration: the declaration with the initializer (if any), its
    /// initializer, and whether it has static storage.
    dtors: FxHashMap<DeclId, (DeclId, Option<ExprId>, bool)>,
    /// The `pass_object_size(t)` arguments with their `t` ([`Folds::obj_size`]).
    ps_args: Vec<(ExprId, u32)>,
    /// WP-1008: the array initializer lists clang's `EmitArrayInit` may
    /// copy from a `constinit` global, in pre-order, each with the
    /// nearest enclosing initializer whose folded value would be emitted
    /// in its place ([`Collect::cover`]).
    arr_lists: Vec<(ExprId, Option<ExprId>)>,
    /// The innermost enclosing initializer that covers the expressions
    /// being walked: an array list, or a variable initializer whose value
    /// is emitted as a constant when it folds.
    cover: Option<ExprId>,
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
                        // Its contract assertions (spec/contracts.md §6.3).
                        if ast.contract_semantic.checks()
                            && let Some(l) = ast.function_contracts(d)
                        {
                            for c in ast.lists.iter::<StmtId>(l) {
                                self.stmt(c);
                            }
                        }
                    }
                }
                DeclKind::Var if !is_local(ast, d) => self.var(d, true),
                // A default member initializer's immediate invocations
                // are left unevaluated by sema, like a default argument's.
                // Its other candidates are folded once, shared by every
                // constructor that uses it (clang folds `int m = c;` with
                // `static const int c = 5;` to `store i32 5`).
                DeclKind::Field => {
                    if let Some(init) = ast.decls.field(d).and_then(|f| f.init) {
                        self.immediates(init, None, 0);
                        let n = ast.exprs.get(init);
                        if !n.is_type_dependent() && !n.is_value_dependent() {
                            self.expr(init);
                        }
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
        let Some(ty) = ast.decls.get(d).ty() else { return };
        if !ast.types.is_reference(ty) && !ast.types.is_dependent(ty) && ast.needs_destruction(ty) {
            let e = self.dtors.entry(ast.first(d)).or_insert((d, v.init, is_static));
            if e.1.is_none() {
                *e = (d, v.init, is_static);
            }
        }
        let Some(init) = v.init else { return };
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
        // An automatic aggregate's value is used only when it is POD
        // (clang `IsConstantAggregate`, `Cg::emit_var_init`) or, for
        // its destruction, when it is `const` ([expr.const]/7,
        // `Oven::has_constant_destruction`): any other is not evaluated,
        // which would build its whole object once per variable (a
        // non-trivial class holding a large class by value, WP-633).
        let fold = constant_init
            || match ast.types.canonical_view(ty) {
                TypeView::ConstantArray(_) | TypeView::Record(_) => {
                    quals.is_const() || crate::class::is_pod(ast, ty)
                }
                TypeView::LValueRef(_) | TypeView::RValueRef(_) => true,
                _ => false,
            };
        if fold {
            self.out.push(init);
            if let Some(x) = flexible_elements_init(ast, init) {
                self.out.push(x);
            }
        }
        if constant_init {
            self.inits.insert(init, d);
        }
        if is_static && ast.lang_c {
            self.c_static.insert(init);
        }
        // The folded value replaces the whole initializer when the
        // variable is static or an automatic aggregate clang emits as a
        // constant (`IsConstantAggregate`, `Cg::emit_var_init`), so no
        // array list inside it is emitted. Any other variable (one in a
        // statement expression inside a list, say) starts uncovered.
        let covers = is_static
            || (v.spec.is_constexpr() && !ast.types.is_reference(ty))
            || (matches!(
                ast.types.canonical_view(ty),
                TypeView::ConstantArray(_) | TypeView::Record(_)
            ) && crate::class::is_pod(ast, ty));
        let saved = std::mem::replace(&mut self.cover, covers.then_some(init));
        self.expr(init);
        self.cover = saved;
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
                        let Some(e) = <ExprId as kiln_ast::ids::RawId>::from_raw(node.a) else {
                            continue;
                        };
                        match node.kind {
                            kiln_ast::attr::AttrKind::ASSUME => self.expr(e),
                            // A loop hint's value is folded, not emitted
                            // (clang `EvaluateKnownConstInt` in
                            // `LoopInfoStack::push`).
                            kiln_ast::attr::AttrKind::LOOP_HINT => self.out.push(e),
                            _ => {}
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
            StmtKind::CXXExpansionStmt if ast.stmts.expansion_expanded(s) => {
                let d = *ast.stmts.expansion_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                let l = &ast.lists;
                for x in l.iter::<StmtId>(d.prelude).chain(l.iter::<StmtId>(d.expansions)) {
                    self.stmt(x);
                }
            }
            StmtKind::ReturnStmt => {
                if let Some(v) = ast.stmts.return_value(s) {
                    self.expr(v);
                }
            }
            StmtKind::IndirectGotoStmt => self.expr(ast.stmts.indirect_goto_target(s)),
            StmtKind::ContractAssertStmt if ast.contract_semantic.checks() => {
                self.expr(ast.stmts.contract_cond(s));
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
            // Every piece of a coroutine body is emitted (spec/codegen.md
            // §7o).
            StmtKind::CoroutineBodyStmt => {
                let c = *ast.stmts.coroutine_body(s);
                for m in ast.lists.iter::<StmtId>(c.param_moves).collect::<Vec<_>>() {
                    self.stmt(m);
                }
                self.stmt(c.promise_stmt);
                self.expr(c.allocate);
                self.expr(c.deallocate);
                self.expr(c.return_value);
                c.result_decl.into_iter().for_each(|d| self.local_decl(d));
                c.return_stmt.into_iter().for_each(|x| self.stmt(x));
                c.return_on_alloc_failure.into_iter().for_each(|x| self.stmt(x));
                self.expr(c.init_suspend);
                self.stmt(c.body);
                c.on_exception.into_iter().for_each(|x| self.stmt(x));
                c.on_fallthrough.into_iter().for_each(|x| self.stmt(x));
                self.expr(c.final_suspend);
            }
            StmtKind::CoreturnStmt => {
                ast.stmts.coreturn_operand(s).into_iter().for_each(|x| self.expr(x));
                ast.stmts.coreturn_promise_call(s).into_iter().for_each(|x| self.expr(x));
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
            let is_static = is_static_local(ast, d)
                || ast.decls.var(d).is_some_and(|v| v.spec.storage() == VarStorage::Extern);
            self.var(d, is_static);
            // A tuple-like decomposition's holding variables are not in
            // the `DeclStmt` ([dcl.struct.bind]/4): a `static constexpr`
            // one's are constant-initialized like `e` (clang emits
            // `@_ZZ1fvE1x = internal constant ptr @_ZZ1fvEDC1x1yE`).
            if let Some(l) = ast.decls.decomp_bindings(d) {
                for b in ast.lists.iter::<DeclId>(l) {
                    if let Some(h) = ast.decls.binding_var(b) {
                        self.var(h, is_static_local(ast, h));
                    }
                }
            }
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
            // `tryEmitAsConstant` on a scalar member access, then
            // `EvaluateAsInt` on an integral one.
            ExprKind::MemberExpr if is_scalar_member(ast, n.ty()) => {
                let integral = ast.types.is_integral(n.ty())
                    || matches!(ast.types.canonical_view(n.ty()), TypeView::Enum(_));
                // clang `tryEmitAsConstant(MemberExpr)` converts a named
                // `const` static data member to its variable first
                // (`tryToConvertMemberExprToDeclRefExpr`): the value is the
                // variable's, whatever the base (`this->b.w`), for any
                // scalar type (`static constexpr const char *name`, WP-940).
                let d = ast.exprs.decl(e);
                let stat = ast.decls.get(d).kind() == DeclKind::Var
                    && ast.decls.get(d).ty().is_some_and(|t| {
                        let q = ast.types.effective_quals(t);
                        q.is_const() && !q.is_volatile()
                    });
                if integral || stat || names_reflection_holder(ast, e) {
                    self.out.push(e);
                }
                if stat && let Some(init) = ast.decls.var(d).and_then(|v| v.init) {
                    self.out.push(init);
                    self.inits.insert(init, d);
                    self.statics.push((e, init));
                }
            }
            // clang `EmitBuiltinExpr` first tries the call as a constant.
            ExprKind::CallExpr => {
                if crate::builtins::is_fold_candidate(ast, e, self.compat) {
                    self.out.push(e);
                }
                self.out.extend(crate::builtins::folded_args(ast, e));
                self.ps_args.extend(crate::builtins::object_size_args(ast, e));
            }
            ExprKind::CXXMemberCallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::CXXConstructExpr
            | ExprKind::CXXTemporaryObjectExpr => {
                self.ps_args.extend(crate::builtins::object_size_args(ast, e));
            }
            // clang `AggExprEmitter::EmitArrayInit` tries an array of
            // trivially copyable elements as a constant (WP-1008); the size
            // condition needs the layout ([`array_lists`]).
            ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
                if ast.types.is_array(n.ty())
                    && !n.is_type_dependent()
                    && !n.is_value_dependent()
                    && crate::class::is_trivially_copyable(ast, n.ty()) =>
            {
                self.arr_lists.push((e, self.cover));
                let saved = self.cover.replace(e);
                for c in ast.exprs.children(e, &ast.lists) {
                    self.expr(c);
                }
                self.cover = saved;
                return;
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
            // clang `ScalarExprEmitter::VisitArraySubscriptExpr` visits a
            // vector base as a scalar: `VisitDeclRefExpr`'s
            // `tryEmitAsConstant` folds a `const` vector variable, and the
            // lane is an `extractelement` of the constant (WP-453).
            ExprKind::ArraySubscriptExpr => {
                for b in ast.exprs.operands(e) {
                    if matches!(
                        ast.types.canonical_view(ast.exprs.get(b).ty()),
                        TypeView::Vector { .. }
                    ) && refers_to_const_var(ast, b)
                    {
                        self.out.push(b);
                    }
                }
            }
            // clang `ScalarExprEmitter::VisitConvertVectorExpr` visits its
            // glvalue operand as a scalar: a `const` vector variable folds
            // through `tryEmitAsConstant` like a subscript base (WP-490).
            ExprKind::ConvertVectorExpr => {
                let b = ast.exprs.operand(e);
                if matches!(
                    ast.types.canonical_view(ast.exprs.get(b).ty()),
                    TypeView::Vector { .. }
                ) && refers_to_const_var(ast, b)
                {
                    self.out.push(b);
                }
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
                _ => ast.exprs.children_into(x, &ast.lists, &mut stack),
            }
        }
    }
}

/// Whether `e` (through parentheses and no-op casts) names a variable of const,
/// non-volatile type, or a reference to one (clang
/// `checkVarTypeForConstantEmission`: a reference whose referent type is
/// const-emittable is `CEK_AsValueOrReference`, so `tryEmitAsConstant`
/// folds its value when the referent is constant; `const int& a = {}` at
/// namespace scope reads as `0`, WP-233).
pub(crate) fn refers_to_const_var(ast: &Ast, mut e: ExprId) -> bool {
    // Through parentheses and no-op casts (the xvalue of an implicitly
    // movable `return x;`): clang's `VisitCastExpr` visits a `CK_NoOp`
    // operand, reaching `VisitDeclRefExpr`'s `tryEmitAsConstant`. And
    // through a reference template argument (`R` of `template <const
    // int &R>` naming `static const int cg = 3;`): clang's
    // `VisitSubstNonTypeTemplateParmExpr` visits the replacement, the
    // `DeclRefExpr` (WP-998).
    loop {
        match ast.exprs.get(e).kind() {
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            ExprKind::SubstNonTypeTemplateParmExpr => e = ast.exprs.subst_replacement(e),
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

/// Whether member access `e` reads a subobject of a constant variable
/// whose value holds a reflection (`gs.name` of `constexpr S gs{{},
/// "x"}`): that variable has no run-time object (spec/codegen.md
/// "Consteval-only variables"), so the read must fold.
fn names_reflection_holder(ast: &Ast, mut e: ExprId) -> bool {
    loop {
        match ast.exprs.get(e).kind() {
            ExprKind::MemberExpr if !ast.exprs.get(e).has(ExprFlags::ARROW) => {
                e = ast.exprs.operand(e);
            }
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
    ast.decls.get(d).kind() == DeclKind::Var
        && ast.object_value(d).is_some_and(|c| ast.holds_reflection(c))
}

/// Whether local variable `d` has static or thread storage duration
/// (a function-local `static` or `thread_local`; [basic.stc.thread]/1:
/// block-scope `thread_local` implies `static`).
pub(crate) fn is_static_local(ast: &Ast, d: DeclId) -> bool {
    is_local(ast, d)
        && ast.decls.var(d).is_some_and(|v| {
            v.spec.storage() == VarStorage::Static || v.spec.tls() != kiln_ast::decl::TlsKind::None
        })
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
