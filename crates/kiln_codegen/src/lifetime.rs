//! Lifetime markers (spec/codegen.md §7p): clang's `EmitLifetimeStart` /
//! `CallLifetimeEnd` and its `VarBypassDetector`.
//!
//! clang emits `llvm.lifetime.start`/`.end` around local variables and
//! temporaries when optimizing (`ShouldEmitLifetimeMarkers`: -O1 and up),
//! and in a coroutine always (`CoroSplit` uses them to keep short-lived
//! locals out of the frame); kiln does the same
//! ([`Cg::lifetime_markers_on`]). A variable that a `goto` or a `switch`
//! jumps past gets none, because its lifetime would be broken into
//! several regions.

use kiln_ast::decl::VarStorage;
use kiln_ast::expr::{ExprKind, StorageDuration};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_llvm::{Attr, AttrKind, AttrList, CallOpts, TypeId as LTy, Value, intrinsics};
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::cleanup::Cleanup;
use crate::fir::{Op, V};
use crate::module::Cg;

/// The variables whose lifetime a jump bypasses (clang
/// `VarBypassDetector`).
#[derive(Debug, Default)]
pub(crate) struct Bypasses {
    /// An indirect `goto` may bypass any variable (`AlwaysBypassed`).
    always: bool,
    vars: FxHashSet<DeclId>,
}

impl Bypasses {
    /// `IsBypassed`.
    pub fn is_bypassed(&self, d: DeclId) -> bool {
        self.always || self.vars.contains(&d)
    }
}

/// `VarBypassDetector::Init`'s walk.
struct Detector<'a> {
    ast: &'a kiln_ast::Ast,
    /// Each scope's parent and variable (`Scopes`); scope 0 is the body.
    scopes: Vec<(u32, Option<DeclId>)>,
    /// gotos and switches with their scope (`FromScopes`).
    from: Vec<(StmtId, u32)>,
    /// The scope of each label, case and default (`ToScopes`).
    to: FxHashMap<StmtId, u32>,
    labels: FxHashMap<DeclId, StmtId>,
}

/// `VarBypassDetector::Init` over the function body `body`.
pub(crate) fn detect_bypasses(ast: &kiln_ast::Ast, body: StmtId) -> Bypasses {
    let mut d = Detector {
        ast,
        scopes: vec![(u32::MAX, None)],
        from: Vec::new(),
        to: FxHashMap::default(),
        labels: FxHashMap::default(),
    };
    let mut parent = 0;
    if !d.stmt(body, &mut parent) {
        return Bypasses { always: true, vars: FxHashSet::default() };
    }
    let mut out = Bypasses::default();
    let st = &ast.stmts;
    for &(s, from) in &d.from {
        if st.get(s).kind() == StmtKind::GotoStmt {
            if let Some(&to) = d.labels.get(&st.label_decl(s)).and_then(|l| d.to.get(l)) {
                d.detect(from, to, &mut out.vars);
            }
        } else {
            for c in ast.lists.iter::<StmtId>(st.switch_data(s).cases) {
                if let Some(&to) = d.to.get(&c) {
                    d.detect(from, to, &mut out.vars);
                }
            }
        }
    }
    out
}

impl Detector<'_> {
    /// `Detect(From, To)`: the variables entered on the way to `to`.
    fn detect(&self, mut from: u32, mut to: u32, out: &mut FxHashSet<DeclId>) {
        while from != to {
            if from < to {
                let (p, v) = self.scopes[to as usize];
                out.extend(v);
                to = p;
            } else {
                from = self.scopes[from as usize].0;
            }
        }
    }

    /// `BuildScopeInformation(Decl*)`.
    fn decl(&mut self, d: DeclId, parent: &mut u32) -> bool {
        let Some(v) = self.ast.decls.var(d) else { return true };
        if !matches!(v.spec.storage(), VarStorage::Static | VarStorage::Extern) {
            self.scopes.push((*parent, Some(d)));
            *parent = u32::try_from(self.scopes.len() - 1).expect("scope count fits u32");
        }
        v.init.is_none_or(|i| self.expr(i))
    }

    fn opt_stmt(&mut self, s: Option<StmtId>, parent: &mut u32) -> bool {
        s.is_none_or(|s| self.stmt(s, parent))
    }

    fn opt_expr(&mut self, e: Option<ExprId>) -> bool {
        e.is_none_or(|e| self.expr(e))
    }

    /// `BuildScopeInformation(Stmt*)` for a statement: scopes opened in
    /// it end with it, except a declaration statement's, which extend to
    /// the end of the enclosing block (`parent`). `false` when an
    /// indirect `goto` makes the analysis give up.
    fn stmt(&mut self, mut s: StmtId, parent: &mut u32) -> bool {
        let ast = self.ast;
        let st = &ast.stmts;
        // Cases, defaults and labels are not scope parents.
        loop {
            let n = *st.get(s);
            let next = match n.kind() {
                StmtKind::CaseStmt => st.case_data(s).sub,
                StmtKind::DefaultStmt => st.default_sub(s),
                StmtKind::LabelStmt => {
                    self.labels.insert(st.label_decl(s), s);
                    StmtId::new(n.b())
                }
                _ => break,
            };
            self.to.insert(s, *parent);
            s = next;
        }
        let kind = st.get(s).kind();
        if kind == StmtKind::DeclStmt {
            let ds: Vec<DeclId> = ast.lists.iter(st.decl_stmt_decls(s)).collect();
            return ds.into_iter().all(|d| self.decl(d, parent));
        }
        let mut local = *parent;
        let p = &mut local;
        match kind {
            StmtKind::IndirectGotoStmt => false,
            StmtKind::GotoStmt => {
                self.from.push((s, *p));
                true
            }
            StmtKind::CompoundStmt => {
                let cs: Vec<StmtId> = ast.lists.iter(st.compound_body(s)).collect();
                cs.into_iter().all(|c| self.stmt(c, p))
            }
            StmtKind::ExprStmt => self.expr(st.expr_stmt_expr(s)),
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                self.opt_stmt(d.init, p)
                    && d.cond_var.is_none_or(|v| self.decl(v, p))
                    && self.opt_expr(d.cond)
                    && self.opt_stmt(d.then, p)
                    && self.opt_stmt(d.els, p)
            }
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                if !(self.opt_stmt(d.init, p) && d.cond_var.is_none_or(|v| self.decl(v, p))) {
                    return false;
                }
                self.from.push((s, *p));
                self.expr(d.cond) && self.stmt(d.body, p)
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                let d = *st.loop_data(s);
                self.opt_stmt(d.init, p)
                    && d.cond_var.is_none_or(|v| self.decl(v, p))
                    && self.opt_expr(d.cond)
                    && self.opt_expr(d.inc)
                    && self.opt_stmt(d.body, p)
            }
            StmtKind::CXXForRangeStmt => {
                let d = *st.range_for_data(s);
                self.opt_stmt(d.init, p)
                    && self.decl(d.range, p)
                    && d.begin.is_none_or(|b| self.decl(b, p))
                    && d.end.is_none_or(|e| self.decl(e, p))
                    && self.opt_expr(d.cond)
                    && self.opt_expr(d.inc)
                    && self.decl(d.loop_var, p)
                    && self.stmt(d.body, p)
            }
            StmtKind::CXXExpansionStmt if st.expansion_expanded(s) => {
                let d = *st.expansion_data(s);
                let l = &self.ast.lists;
                let kids: Vec<StmtId> =
                    l.iter(d.prelude).chain(l.iter::<StmtId>(d.expansions)).collect();
                self.opt_stmt(d.init, p) && kids.into_iter().all(|x| self.stmt(x, p))
            }
            StmtKind::AttributedStmt => self.stmt(StmtId::new(st.get(s).b()), p),
            StmtKind::ReturnStmt => self.opt_expr(st.return_value(s)),
            StmtKind::CoreturnStmt => self.opt_expr(st.coreturn_operand(s)),
            StmtKind::CXXTryStmt => {
                if !self.stmt(st.try_block(s), p) {
                    return false;
                }
                let hs: Vec<StmtId> = ast.lists.iter(st.try_handlers(s)).collect();
                hs.into_iter().all(|h| {
                    let mut hp = *p;
                    self.stmt(st.catch_block(h), &mut hp)
                })
            }
            _ => true,
        }
    }

    /// `BuildScopeInformation(Stmt*)` for an expression: only a GNU
    /// statement expression holds statements.
    fn expr(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut stack = vec![e];
        while let Some(e) = stack.pop() {
            if ast.exprs.get(e).kind() == ExprKind::StmtExpr {
                // The enclosing statement's scope does not matter: a
                // jump into a statement expression is ill-formed.
                let mut p = 0;
                if !self.stmt(ast.exprs.stmt_expr_body(e), &mut p) {
                    return false;
                }
                continue;
            }
            ast.exprs.children_into(e, &ast.lists, &mut stack);
        }
        true
    }
}

impl Cg<'_> {
    /// clang `ShouldEmitLifetimeMarkers`: optimizing, an address
    /// sanitizer checking use after scope, a hardware-address or memory
    /// sanitizer, or in a coroutine.
    pub(crate) fn lifetime_markers_on(&self) -> bool {
        use kiln_ast::sanitize::SanitizerSet as S;
        let san = &self.opts.sanitize;
        self.opts.optimize()
            || self.f.lifetime_markers
            || san.use_after_scope
            || san.kinds.has_any(S::HWADDRESS.union(S::MEMORY))
    }

    /// clang `EmitLifetimeStart` of the alloca `addr` of type `ty`: the
    /// size when a marker was emitted.
    pub(crate) fn emit_lifetime_start(&mut self, addr: V, ty: LTy) -> Option<u64> {
        if !self.lifetime_markers_on() {
            return None;
        }
        let size = self.alloc_size(ty);
        let op = self.lifetime_call(true, addr, size);
        self.f.fir.push(op, LTy::VOID);
        Some(size)
    }

    /// clang `EmitLifetimeEnd`.
    pub(crate) fn emit_lifetime_end(&mut self, addr: V, size: u64) {
        let op = self.lifetime_call(false, addr, size);
        self.f.fir.push(op, LTy::VOID);
    }

    /// A `nounwind` call of `llvm.lifetime.start`/`.end`.
    fn lifetime_call(&mut self, start: bool, addr: V, size: u64) -> Op {
        let f = if start {
            intrinsics::lifetime_start(&mut self.m)
        } else {
            intrinsics::lifetime_end(&mut self.m)
        };
        let fn_ty = self.m.func(f).ty();
        let fa = self.m.attr_set(&[Attr::Flag(AttrKind::NoUnwind)]);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        // LLVM 22+ (`ir_version`, clang 22 at compat 22): no size operand.
        let args = if intrinsics::lifetime_has_size(&self.m) {
            vec![V::C(self.m.const_int(LTy::I64, i128::from(size))), addr]
        } else {
            vec![addr]
        };
        Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::new(opts) }
    }

    /// The lifetime start and end of the automatic variable `d` at the
    /// alloca `addr` (the end of clang `EmitAutoVarAlloca`).
    pub(crate) fn var_lifetime(&mut self, d: DeclId, addr: V, ty: LTy) {
        if self.f.fir.cur().is_none() || self.f.bypasses.as_ref().is_some_and(|b| b.is_bypassed(d))
        {
            return;
        }
        if let Some(size) = self.emit_lifetime_start(addr, ty) {
            self.push_cleanup(Cleanup::LifetimeEnd { addr, size });
        }
    }
}

impl Cg<'_> {
    /// The lifetime of the materialized temporary `m` (operand `s`) at
    /// `tmp` (clang `EmitMaterializeTemporaryExpr`): a lifetime-extended
    /// one ends after the full-expression's cleanups; a full-expression
    /// one whose type has no destructor starts before the outermost
    /// conditional, so its end needs no `cleanup.cond` flag.
    pub(crate) fn temporary_lifetime(&mut self, m: ExprId, s: ExprId, tmp: crate::func::Addr) {
        if !self.lifetime_markers_on() || self.f.fir.cur().is_none() {
            return;
        }
        let ast = self.ast;
        let size = self.alloc_size(tmp.ty);
        let end = Cleanup::LifetimeEnd { addr: tmp.v, size };
        match ast.exprs.temporary_storage(m) {
            StorageDuration::Automatic => {
                self.emit_lifetime_start(tmp.v, tmp.ty);
                let flag = (!self.f.conds.is_empty()).then(|| self.create_cleanup_active_flag());
                self.f.cl.lifetime_extended.push((end, flag));
            }
            StorageDuration::FullExpression => {
                let destructed = self.destructed_class(ast.exprs.get(s).ty()).is_some();
                if let Some(&outer) = self.f.conds.first()
                    && !destructed
                {
                    // clang moves the builder to the outermost
                    // conditional's start (an access group, unlike the
                    // `new StoreInst` inserts of the cleanup flags).
                    let op = self.lifetime_call(true, tmp.v, size);
                    self.f.fir.builder_insert_before_last(outer, op, LTy::VOID);
                    let conds = std::mem::take(&mut self.f.conds);
                    self.push_full_expr_cleanup(end, true, true);
                    self.f.conds = conds;
                } else {
                    self.emit_lifetime_start(tmp.v, tmp.ty);
                    self.push_full_expr_cleanup(end, true, true);
                }
            }
            StorageDuration::Static | StorageDuration::Thread => {}
        }
    }
}
