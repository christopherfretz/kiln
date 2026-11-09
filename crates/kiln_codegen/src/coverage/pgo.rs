//! clang 19 `CodeGenPGO`'s region counters (WP-941, spec/codegen.md
//! "Instrumentation"): `MapRegionCounters` numbers the statements and
//! expressions that get an `llvm.instrprof.increment` counter, in the
//! pre-order of a `RecursiveASTVisitor` that does not visit implicit code,
//! and folds the shape of the body into the V3 `PGOHash`.
//!
//! The numbering and the hash must equal clang's for `llvm-profdata merge`
//! and `llvm-cov` to treat kiln's records as clang's would: the walk
//! below mirrors `RecursiveASTVisitor`'s traversal order statement by
//! statement (including its quirks: a local class's member function
//! bodies and a block-scope function declaration take counters of the
//! enclosing function; a lambda's body is not walked; a defaulted
//! function's body is not walked).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{Ast, DeclId, ExprId, ListId, StmtId};
use kiln_llvm::{FuncId, GlobalId, Linkage, TypeId as LTy, Value, Visibility};
use kiln_source::fx::FxHashMap;

use crate::fir::{Blk, V};
use crate::module::Cg;

/// What a counter is attached to (clang's `const Stmt *` key).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Key {
    Stmt(StmtId),
    Expr(ExprId),
}

/// The counters of one function (clang `RegionCounterMap`,
/// `NumRegionCounters`, `FunctionHash`).
#[derive(Debug, Default)]
pub(crate) struct RegionCounters {
    pub map: FxHashMap<Key, u32>,
    pub num: u32,
    pub hash: u64,
}

/// `PGOHash::HashType` (V3).
#[derive(Clone, Copy)]
#[repr(u8)]
enum H {
    Label = 1,
    While,
    Do,
    For,
    ForRange,
    Switch = 7,
    Case,
    Default,
    If,
    Try,
    Catch,
    CondOp,
    LAnd,
    LOr,
    BinaryCondOp,
    EndOfScope,
    IfThen,
    IfElse,
    Goto,
    IndirectGoto,
    Break,
    Continue,
    Return,
    Throw,
    LNot,
    LT,
    GT,
    LE,
    GE,
    EQ,
    NE,
}

/// clang `PGOHash`: six bits per type, ten types per word, the words
/// through MD5 once there are more than ten.
#[derive(Default)]
struct PgoHash {
    count: u64,
    working: u64,
    words: Vec<u8>,
}

impl PgoHash {
    fn combine(&mut self, t: H) {
        if self.count != 0 && self.count.is_multiple_of(10) {
            self.words.extend_from_slice(&self.working.to_le_bytes());
            self.working = 0;
        }
        self.count += 1;
        self.working = (self.working << 6) | u64::from(t as u8);
    }

    fn finalize(mut self) -> u64 {
        if self.count <= 10 {
            return self.working;
        }
        if self.working != 0 {
            self.words.extend_from_slice(&self.working.to_le_bytes());
        }
        let d = crate::debuginfo::md5::digest(&self.words);
        u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])
    }
}

/// clang `CodeGenFunction::stripCond` + `isInstrumentedCondition`: `c`
/// (through parentheses and `!`) is not a `&&` or `||`.
pub(crate) fn is_instrumented_condition(ast: &Ast, mut c: ExprId) -> bool {
    let ex = &ast.exprs;
    loop {
        c = ignore_parens(ast, c);
        if ex.get(c).kind() == ExprKind::UnaryOperator && ex.unary_op(c) == UnaryOp::LNot {
            c = ex.operand(c);
        } else {
            break;
        }
    }
    !(ex.get(c).kind() == ExprKind::BinaryOperator
        && matches!(ex.binary_op(c), BinaryOp::LAnd | BinaryOp::LOr))
}

/// clang `Expr::IgnoreParens` (parentheses and `__extension__`).
fn ignore_parens(ast: &Ast, mut e: ExprId) -> ExprId {
    let ex = &ast.exprs;
    loop {
        match ex.get(e).kind() {
            ExprKind::ParenExpr => e = ex.operand(e),
            ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::Extension => {
                e = ex.operand(e);
            }
            _ => return e,
        }
    }
}

/// clang `CodeGenPGO::mapRegionCounters` for the function definition `d`.
pub(crate) fn map_region_counters(ast: &Ast, d: DeclId) -> RegionCounters {
    let mut w = Walker { ast, map: FxHashMap::default(), next: 0, hash: PgoHash::default() };
    w.function(d);
    if let Some(l) = ast.function_contracts(d) {
        // P2900 pre/postconditions (no clang 19 counterpart): walked
        // after the body so a function without them hashes as clang's.
        let cs: Vec<StmtId> = ast.lists.iter(l).collect();
        for s in cs {
            w.stmt(s);
        }
    }
    RegionCounters { map: w.map, num: w.next, hash: w.hash.finalize() }
}

struct Walker<'a> {
    ast: &'a Ast,
    map: FxHashMap<Key, u32>,
    next: u32,
    hash: PgoHash,
}

impl Walker<'_> {
    fn counter(&mut self, k: Option<Key>) {
        if let Some(k) = k {
            self.map.insert(k, self.next);
        }
        self.next += 1;
    }

    /// `TraverseDecl` of a function: `VisitDecl` (the body's counter),
    /// the parameters' default arguments (`TraverseTypeLoc`), a
    /// constructor's written mem-initializers, and the body unless the
    /// function is defaulted.
    fn function(&mut self, d: DeclId) {
        let ast = self.ast;
        let Some(fd) = ast.decls.function(d).copied() else { return };
        self.counter(fd.body.map(Key::Stmt));
        let params: Vec<DeclId> = ast.lists.iter(fd.params).collect();
        for p in params {
            if let Some(a) = ast.decls.param(p).and_then(|x| x.default_arg) {
                self.expr(a);
            }
        }
        if ast.decls.get(d).kind() == DeclKind::CXXConstructor {
            let inits: Vec<ExprId> =
                ast.ctor_inits(d).filter(|i| i.is_written()).map(|i| i.init).collect();
            for i in inits {
                self.expr(i);
            }
        }
        if let Some(b) = fd.body
            && fd.spec.defaulted() == kiln_ast::decl::Defaulted::No
        {
            self.stmt(b);
        }
    }

    /// `TraverseDecl` of a declaration in a `DeclStmt`.
    fn decl(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if n.has(kiln_ast::decl::DeclFlags::IMPLICIT) {
            return;
        }
        let kind = n.kind();
        if kind.is_function() {
            self.function(d);
        } else if let Some(v) = ast.decls.var(d) {
            if let Some(i) = v.init {
                self.expr(i);
            }
        } else if let Some(f) = ast.decls.field(d) {
            if let Some(i) = f.init {
                self.expr(i);
            }
        } else if let Some(s) = ast.decls.static_assert(d) {
            let (c, m) = (s.cond, s.message);
            self.expr(c);
            if let Some(m) = m {
                self.expr(m);
            }
        } else if kind == DeclKind::CXXRecord || ast.decls.record(d).is_some() {
            let ms: Vec<DeclId> = ast.members(d).collect();
            for m in ms {
                self.decl(m);
            }
        }
    }

    fn opt_stmt(&mut self, s: Option<StmtId>) {
        if let Some(s) = s {
            self.stmt(s);
        }
    }

    fn opt_expr(&mut self, e: Option<ExprId>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    fn stmts(&mut self, l: ListId) {
        let v: Vec<StmtId> = self.ast.lists.iter(l).collect();
        for s in v {
            self.stmt(s);
        }
    }

    /// `TraverseStmt` of a statement: `VisitStmt`, then the children in
    /// `RecursiveASTVisitor` order, then `EndOfScope` for the nestable
    /// statements.
    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        let st = &ast.stmts;
        let kind = st.get(s).kind();
        let (counted, h) = match kind {
            StmtKind::LabelStmt => (true, Some(H::Label)),
            StmtKind::WhileStmt => (true, Some(H::While)),
            StmtKind::DoStmt => (true, Some(H::Do)),
            StmtKind::ForStmt => (true, Some(H::For)),
            StmtKind::CXXForRangeStmt => (true, Some(H::ForRange)),
            StmtKind::SwitchStmt => (true, Some(H::Switch)),
            StmtKind::CaseStmt => (true, Some(H::Case)),
            StmtKind::DefaultStmt => (true, Some(H::Default)),
            StmtKind::IfStmt => (true, Some(H::If)),
            StmtKind::CXXTryStmt => (true, Some(H::Try)),
            StmtKind::CXXCatchStmt => (true, Some(H::Catch)),
            StmtKind::GotoStmt => (false, Some(H::Goto)),
            StmtKind::IndirectGotoStmt => (false, Some(H::IndirectGoto)),
            StmtKind::BreakStmt => (false, Some(H::Break)),
            StmtKind::ContinueStmt => (false, Some(H::Continue)),
            StmtKind::ReturnStmt => (false, Some(H::Return)),
            _ => (false, None),
        };
        if counted {
            self.counter(Some(Key::Stmt(s)));
        }
        if let Some(h) = h {
            self.hash.combine(h);
        }
        let mut end_scope = false;
        match kind {
            StmtKind::CompoundStmt => self.stmts(st.compound_body(s)),
            StmtKind::DeclStmt => {
                let ds: Vec<DeclId> = ast.lists.iter(st.decl_stmt_decls(s)).collect();
                for d in ds {
                    self.decl(d);
                }
            }
            StmtKind::ExprStmt => self.expr(st.expr_stmt_expr(s)),
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                self.opt_stmt(d.init);
                if let Some(v) = d.cond_var {
                    self.decl(v);
                }
                self.opt_expr(d.cond);
                if let Some(t) = d.then {
                    self.hash.combine(H::IfThen);
                    self.stmt(t);
                }
                if let Some(e) = d.els {
                    self.hash.combine(H::IfElse);
                    self.stmt(e);
                }
                end_scope = true;
            }
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                self.opt_stmt(d.init);
                if let Some(v) = d.cond_var {
                    self.decl(v);
                }
                self.expr(d.cond);
                self.stmt(d.body);
            }
            StmtKind::WhileStmt => {
                let d = *st.loop_data(s);
                if let Some(v) = d.cond_var {
                    self.decl(v);
                }
                self.opt_expr(d.cond);
                self.opt_stmt(d.body);
                end_scope = true;
            }
            StmtKind::DoStmt => {
                let d = *st.loop_data(s);
                self.opt_stmt(d.body);
                self.opt_expr(d.cond);
                end_scope = true;
            }
            StmtKind::ForStmt => {
                let d = *st.loop_data(s);
                self.opt_stmt(d.init);
                if let Some(v) = d.cond_var {
                    self.decl(v);
                }
                self.opt_expr(d.cond);
                self.opt_expr(d.inc);
                self.opt_stmt(d.body);
                end_scope = true;
            }
            StmtKind::CXXForRangeStmt => {
                // Without implicit code: the init-statement, the loop
                // variable (not its initializer), the range initializer
                // and the body.
                let d = *st.range_for_data(s);
                self.opt_stmt(d.init);
                self.opt_expr(ast.decls.var(d.range).and_then(|v| v.init));
                self.stmt(d.body);
                end_scope = true;
            }
            StmtKind::CXXExpansionStmt if st.expansion_expanded(s) => {
                let d = *st.expansion_data(s);
                self.opt_stmt(d.init);
                self.stmts(d.prelude);
                self.stmts(d.expansions);
            }
            StmtKind::CaseStmt => {
                let d = *st.case_data(s);
                self.expr(d.lhs);
                self.opt_expr(d.rhs);
                self.stmt(d.sub);
            }
            StmtKind::DefaultStmt => self.stmt(st.default_sub(s)),
            StmtKind::LabelStmt | StmtKind::AttributedStmt => {
                self.stmt(StmtId::new(st.get(s).b()));
            }
            StmtKind::IndirectGotoStmt => self.expr(st.indirect_goto_target(s)),
            StmtKind::ReturnStmt => self.opt_expr(st.return_value(s)),
            StmtKind::CoreturnStmt => self.opt_expr(st.coreturn_operand(s)),
            StmtKind::CoroutineBodyStmt => self.stmt(st.coroutine_body(s).body),
            StmtKind::CXXTryStmt => {
                self.stmt(st.try_block(s));
                self.stmts(st.try_handlers(s));
                end_scope = true;
            }
            StmtKind::CXXCatchStmt => {
                if let Some(v) = st.catch_var(s) {
                    self.decl(v);
                }
                self.stmt(st.catch_block(s));
                end_scope = true;
            }
            StmtKind::GCCAsmStmt => {
                let es: Vec<ExprId> = st.asm_operands(s).iter().map(|o| o.expr).collect();
                for e in es {
                    self.expr(e);
                }
            }
            StmtKind::ContractAssertStmt => self.expr(st.contract_cond(s)),
            _ => {}
        }
        if end_scope {
            self.hash.combine(H::EndOfScope);
        }
    }

    /// `TraverseStmt` of an expression, in pre-order.
    fn expr(&mut self, root: ExprId) {
        let ast = self.ast;
        let ex = &ast.exprs;
        let mut stack = vec![root];
        let mut kids = Vec::new();
        while let Some(e) = stack.pop() {
            let kind = ex.get(e).kind();
            // VisitStmt.
            match kind {
                ExprKind::ConditionalOperator => {
                    self.counter(Some(Key::Expr(e)));
                    self.hash.combine(H::CondOp);
                }
                ExprKind::BinaryConditionalOperator => {
                    self.counter(Some(Key::Expr(e)));
                    self.hash.combine(H::BinaryCondOp);
                }
                ExprKind::CXXThrowExpr => self.hash.combine(H::Throw),
                ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::LNot => {
                    self.hash.combine(H::LNot);
                }
                ExprKind::BinaryOperator => {
                    let op = ex.binary_op(e);
                    let h = match op {
                        BinaryOp::LAnd => Some(H::LAnd),
                        BinaryOp::LOr => Some(H::LOr),
                        BinaryOp::LT => Some(H::LT),
                        BinaryOp::GT => Some(H::GT),
                        BinaryOp::LE => Some(H::LE),
                        BinaryOp::GE => Some(H::GE),
                        BinaryOp::EQ => Some(H::EQ),
                        BinaryOp::NE => Some(H::NE),
                        _ => None,
                    };
                    let logical = matches!(op, BinaryOp::LAnd | BinaryOp::LOr);
                    if logical {
                        self.counter(Some(Key::Expr(e)));
                    }
                    if let Some(h) = h {
                        self.hash.combine(h);
                    }
                    // VisitBinaryOperator: the right operand of a logical
                    // operator counts its own evaluations (profile v7+).
                    let [_, rhs] = ex.operands(e);
                    if logical && is_instrumented_condition(ast, rhs) {
                        self.counter(Some(Key::Expr(rhs)));
                    }
                }
                _ => {}
            }
            // The children.
            kids.clear();
            match kind {
                ExprKind::StmtExpr => {
                    self.stmt(ex.stmt_expr_body(e));
                    continue;
                }
                ExprKind::CXXRewrittenBinaryOperator => {
                    kids.extend(ast.lists.iter::<ExprId>(ex.rewritten_written(e).1));
                }
                ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                    kids.extend(ex.first_child(e, &ast.lists));
                }
                ExprKind::InitListExpr => {
                    kids.extend(ast.lists.iter::<ExprId>(ex.init_list_inits(e)));
                }
                ExprKind::PseudoObjectExpr => kids.push(ex.pseudo_syntactic(e, &ast.lists)),
                _ => ex.children_into(e, &ast.lists, &mut kids),
            }
            stack.extend(kids.iter().rev());
        }
    }
}

/// The `InstrProf` state of the function being emitted (clang
/// `CodeGenPGO`'s `RegionCounterMap`, `FunctionHash` and `FuncNameVar`).
#[derive(Debug)]
pub(crate) struct ProfState {
    pub rc: RegionCounters,
    /// `@__profn_<name>`.
    pub name: GlobalId,
}

/// LLVM `getPGOFuncName`: a local function's name is prefixed with the
/// main file's name (clang `-main-file-name`, the input's base name).
fn pgo_func_name(raw: &str, linkage: Linkage, main_file: &str) -> String {
    let raw = raw.strip_prefix('\u{1}').unwrap_or(raw);
    if matches!(linkage, Linkage::Internal | Linkage::Private) {
        let file = if main_file.is_empty() { "<unknown>" } else { main_file };
        format!("{file}:{raw}")
    } else {
        raw.to_owned()
    }
}

impl Cg<'_> {
    /// clang `CodeGenPGO::assignRegionCounters` (WP-941): numbers the
    /// counters of the function `d` being emitted as `f` and creates its
    /// `__profn_` name. Not for implicit functions, a constructor or
    /// destructor variant other than the base one (kiln emits a complete
    /// constructor only when delegating to the base one is invalid, as
    /// clang instruments it then), a `noprofile` function, or one in a
    /// system header.
    pub(crate) fn prof_begin(
        &mut self,
        d: DeclId,
        f: FuncId,
        structor: Option<kiln_abi::mangle::Structor>,
    ) {
        if !self.opts.coverage.instr {
            return;
        }
        let ast = self.ast;
        let n = ast.decls.get(d);
        if n.has(kiln_ast::decl::DeclFlags::IMPLICIT)
            || ast.decls.function(d).is_none_or(|x| x.body.is_none())
            || matches!(ast.closure_thunk(d), Some(kiln_ast::lambda::ClosureThunk::Invoker { .. }))
            || n.kind() == DeclKind::CXXDestructor
                && structor != Some(kiln_abi::mangle::Structor::Base)
            || self.has_fn_attr(d, kiln_ast::attr::AttrKind::NO_PROFILE_INSTRUMENT_FUNCTION)
            || self.sm.is_some_and(|sm| sm.is_in_system_header(n.loc()))
        {
            return;
        }
        // `setFuncName` / `createPGOFuncNameVar`.
        let (fname, flink) = {
            let func = self.m.func(f);
            (func.name().to_owned(), func.linkage)
        };
        let main_file = {
            let s = &self.opts.source_filename;
            s.rsplit('/').next().unwrap_or(s).to_owned()
        };
        let name = pgo_func_name(&fname, flink, &main_file);
        let linkage = match flink {
            Linkage::ExternWeak => Linkage::LinkOnce,
            Linkage::AvailableExternally => Linkage::LinkOnceOdr,
            Linkage::Internal | Linkage::External => Linkage::Private,
            l => l,
        };
        let local = matches!(linkage, Linkage::Internal | Linkage::Private);
        let mut var = format!("__profn_{name}");
        if local {
            var = var.chars().map(|c| if "-:;<>/\"'".contains(c) { '_' } else { c }).collect();
        }
        let init = self.m.const_bytes(name.as_bytes());
        let ty = self.m.const_type(init);
        let g = self.m.add_global(&var, ty);
        let _ = self.m.set_initializer(g, Some(init));
        let gv = self.m.global_mut(g);
        gv.linkage = linkage;
        gv.is_constant = true;
        if !local {
            gv.visibility = Visibility::Hidden;
        }
        // `createPGOFuncNameMetadata`.
        if name != fname {
            let k = self.m.md_kind("PGOFuncName");
            let s = self.m.md_string(name.as_bytes());
            let t = self.m.md_tuple(&[Some(s)]);
            self.m.func_mut(f).add_metadata(k, t);
        }
        let mut rc = map_region_counters(ast, d);
        // Clang 20 and later keep the top four bits of a frontend hash
        // clear (`NamedInstrProfRecord::FUNC_HASH_MASK`, reserved for
        // the profile's flags); clang 19 keeps the full MD5 word.
        if self.opts.clang_compat >= 22 {
            rc.hash &= (1 << 60) - 1;
        }
        self.f.prof = Some(Box::new(ProfState { rc, name: g }));
    }

    /// The module-level `InstrProf` pieces (spec/codegen.md
    /// "Instrumentation"): clang `CodeGenModule::Release` puts the
    /// `EnableValueProfiling` flag first, and an explicit
    /// `-fprofile-instr-generate=<path>` becomes the hidden
    /// `__llvm_profile_filename` the runtime reads (clang hands the path
    /// to the lowering pass, which `opt` cannot take, so it is emitted
    /// here exactly as `createProfileFileNameVar` would).
    pub(crate) fn instr_profile_trailer(&mut self) {
        if !self.opts.coverage.instr {
            return;
        }
        if let Some(path) = self.opts.coverage.instr_path.clone() {
            let name = "__llvm_profile_filename";
            let init = self.m.const_cstring(path.as_bytes());
            let ty = self.m.const_type(init);
            let g = self.m.add_global(name, ty);
            let _ = self.m.set_initializer(g, Some(init));
            let c = self.m.comdat(name, kiln_llvm::ComdatKind::Any);
            let gv = self.m.global_mut(g);
            gv.is_constant = true;
            gv.visibility = Visibility::Hidden;
            gv.comdat = Some(c);
        }
        self.m.add_module_flag_i32(2, "EnableValueProfiling", 0);
    }

    /// clang `hasProfileClangInstr()`: `InstrProf` counters are on for the
    /// module (the extra blocks around counters are emitted even in a
    /// function that is not instrumented).
    #[inline]
    pub(crate) fn instr_regions(&self) -> bool {
        self.opts.coverage.instr
    }

    /// clang `incrementProfileCounter(S)`: an `llvm.instrprof.increment`
    /// of the counter of `k` (counter 0 for a key without one, as clang's
    /// `RegionCounterMap[S]`), at an artificial location.
    pub(crate) fn prof_inc(&mut self, k: Key) {
        self.prof_inc_step(k, None);
    }

    /// [`Self::prof_inc`] by `step` (`llvm.instrprof.increment.step`).
    pub(crate) fn prof_inc_step(&mut self, k: Key, step: Option<V>) {
        let Some(p) = self.f.prof.as_deref() else { return };
        if self.f.fir.cur().is_none() {
            return;
        }
        let (g, hash, num) = (p.name, p.rc.hash, p.rc.num);
        let idx = p.rc.map.get(&k).copied().unwrap_or(0);
        let saved = self.dbg_apply(kiln_source::Loc::INVALID);
        let mut args = vec![
            V::C(Value::global(g)),
            V::C(self.m.const_int(LTy::I64, i128::from(hash as i64))),
            V::C(self.m.const_int(LTy::I32, i128::from(num))),
            V::C(self.m.const_int(LTy::I32, i128::from(idx))),
        ];
        let f = match step {
            None => kiln_llvm::intrinsics::instrprof_increment(&mut self.m),
            Some(s) => {
                args.push(s);
                kiln_llvm::intrinsics::instrprof_increment_step(&mut self.m)
            }
        };
        self.intrinsic_call(f, args);
        self.dbg_restore(saved);
    }

    /// clang `EmitBlockWithFallThrough`: `b` starts, with the counter of
    /// `k`; a fall-through into it skips the increment (`skipcount`).
    pub(crate) fn emit_block_with_fallthrough(&mut self, b: Blk, k: Key) {
        let skip = if self.instr_regions() && self.f.fir.cur().is_some() {
            let s = self.f.fir.create_block();
            self.emit_branch(s);
            Some(s)
        } else {
            None
        };
        self.emit_block(b, false);
        self.prof_inc(k);
        if let Some(s) = skip {
            self.emit_block(s, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{H, PgoHash};

    /// clang 19's hashes: `if (x) return 1; return 0;` and a function of
    /// eleven types (through MD5).
    #[test]
    fn pgo_hash() {
        let mut h = PgoHash::default();
        h.combine(H::Return);
        assert_eq!(h.finalize(), 24);
        let h = PgoHash::default();
        assert_eq!(h.finalize(), 0);
    }
}
