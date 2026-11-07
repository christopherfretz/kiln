//! Contract assertions at run time (P2900; spec/contracts.md §6.3): the
//! callee-side checks GCC 16.1 emits, the constant
//! `std::contracts::contract_violation` objects in libstdc++'s layout,
//! and the default violation handler of a translation unit built against
//! a stdlib that ships none.

use kiln_ast::stmt::{ContractKind, ContractSemantic};
use kiln_ast::{DeclId, StmtId};
use kiln_llvm::{
    Attr, AttrKind, CallOpts, GepFlags, IntPredicate, IrBuilder, Linkage, TypeId as LTy, Value,
};

use crate::fir::{Op, V};
use crate::module::{Cg, R, err};

/// `void handle_contract_violation(const std::contracts::contract_violation&)`.
const HANDLER: &str = "_Z25handle_contract_violationRKNSt9contracts18contract_violationE";
/// `void std::contracts::invoke_default_contract_violation_handler(const contract_violation&)`.
const INVOKE_DEFAULT: &str =
    "_ZNSt9contracts41invoke_default_contract_violation_handlerERKNS_18contract_violationE";
/// `std::terminate()`.
const TERMINATE: &str = "_ZSt9terminatev";

/// `std::contracts::detection_mode::predicate_false`.
const PREDICATE_FALSE: u16 = 1;
/// `std::contracts::detection_mode::evaluation_exception`.
const EVALUATION_EXCEPTION: u16 = 2;

/// GCC's default handler message (libstdc++ `contract.cc`).
const DEFAULT_FORMAT: &str = "contract violation in function %s at %s:%u: %s\n\
[assertion_kind: %s, semantic: %s, mode: %s, terminating: %s]\n";

/// The `std::contracts::evaluation_semantic` value of `s`.
fn semantic_value(s: ContractSemantic) -> u16 {
    match s {
        ContractSemantic::Ignore => 1,
        ContractSemantic::Observe => 2,
        ContractSemantic::Enforce => 3,
        ContractSemantic::QuickEnforce => 4,
    }
}

/// The `std::contracts::assertion_kind` value of `k`.
fn kind_value(k: ContractKind) -> u16 {
    match k {
        ContractKind::Pre => 1,
        ContractKind::Post => 2,
        ContractKind::Assert => 3,
    }
}

impl Cg<'_> {
    /// The function contract assertions of `d` of kind `kind`, in order.
    fn contracts_of(&self, d: DeclId, kind: ContractKind) -> Vec<StmtId> {
        let ast = self.ast;
        let Some(l) = ast.function_contracts(d) else { return Vec::new() };
        ast.lists.iter::<StmtId>(l).filter(|&s| ast.stmts.contract_kind(s) == kind).collect()
    }

    /// The preconditions of function `d`, at entry: after the parameters
    /// (and `this`, the VTT, a lambda's captures) and before the body or
    /// a constructor's mem-initializers ([dcl.contract.func]/1).
    pub(crate) fn emit_preconditions(&mut self, d: DeclId) -> R<()> {
        if !self.ast.contract_semantic.checks() {
            return Ok(());
        }
        // Written on another declaration: its parameters are `d`'s.
        for (src, own) in self.ast.contract_param_aliases(d) {
            if let Some(&a) = self.f.locals.get(&own) {
                self.f.locals.insert(src, a);
            }
        }
        for s in self.contracts_of(d, ContractKind::Pre) {
            self.emit_contract_check(s)?;
        }
        Ok(())
    }

    /// The postconditions of function `d` at its (single) normal return,
    /// after the result is initialised and the locals (and a destructor's
    /// members and bases) are destroyed; the result name denotes the
    /// return slot. An exception exit never reaches the return block.
    pub(crate) fn emit_postconditions(&mut self, d: DeclId) -> R<()> {
        if !self.ast.contract_semantic.checks() || self.f.fir.cur().is_none() {
            return Ok(());
        }
        let posts = self.contracts_of(d, ContractKind::Post);
        if let Some(&s) = posts.first()
            && self.f.coro.is_some()
        {
            return err(
                Some(self.ast.stmts.get(s).loc()),
                "unsupported: postcondition of a coroutine (spec/contracts.md §9)",
            );
        }
        for s in posts {
            if let Some(r) = self.ast.stmts.contract_result_name(s) {
                let Some(slot) = self.f.ret_slot else {
                    return err(Some(self.ast.stmts.get(s).loc()), "result name without a result");
                };
                self.f.locals.insert(r, slot);
            }
            self.emit_contract_check(s)?;
        }
        Ok(())
    }

    /// One contract assertion `s` under the translation unit's semantic
    /// ([basic.contract.eval]): the predicate is evaluated inside an
    /// implicit `catch (...)` (a terminate scope under `quick_enforce`);
    /// false is a violation with detection mode `predicate_false`, an
    /// escaping exception one with `evaluation_exception` whose handler
    /// runs inside the implicit handler.
    pub(crate) fn emit_contract_check(&mut self, s: StmtId) -> R<()> {
        let sem = self.ast.contract_semantic;
        if !sem.checks() || self.f.fir.cur().is_none() {
            return Ok(());
        }
        self.contract_checked = true;
        let cond = self.ast.stmts.contract_cond(s);
        let quick = sem == ContractSemantic::QuickEnforce;
        let eh = self.opts.exceptions;
        let done = self.f.fir.create_block();
        let viol = self.f.fir.create_block();
        if eh {
            if quick {
                self.push_terminate();
            } else {
                self.enter_catch_all();
            }
        }
        let sc = self.begin_scope(false);
        let mut c = self.eval_bool(cond)?;
        self.end_scope_reload(sc, &mut [&mut c]);
        if self.f.fir.cur().is_some() {
            self.cond_br(c, done, viol);
            self.f.fir.set_cur(None);
        }
        if eh {
            if quick {
                self.pop_terminate();
            } else {
                self.exit_catch_all_to(Some(done), |cg| {
                    cg.contract_violation(s, EVALUATION_EXCEPTION)
                })?;
            }
        }
        if self.f.fir.block_uses(viol) > 0 {
            self.emit_block(viol, false);
            self.contract_violation(s, PREDICATE_FALSE)?;
            if self.f.fir.cur().is_some() {
                self.br(done);
            }
        }
        self.emit_block(done, true);
        Ok(())
    }

    /// A violation of `s` detected by `mode`: the handler (not under
    /// `quick_enforce`), then `std::terminate()` unless `observe`.
    fn contract_violation(&mut self, s: StmtId, mode: u16) -> R<()> {
        let sem = self.ast.contract_semantic;
        if sem != ContractSemantic::QuickEnforce {
            let obj = self.contract_violation_object(s, mode)?;
            let h = self.rt_fn(HANDLER, LTy::VOID, &[LTy::PTR], &[]);
            self.runtime_call_or_invoke(h, LTy::VOID, vec![V::C(obj)], &[]);
        }
        if sem != ContractSemantic::Observe && self.f.fir.cur().is_some() {
            let t = self.rt_fn(TERMINATE, LTy::VOID, &[], &[]);
            self.nounwind_runtime_call(t, LTy::VOID, Vec::new());
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            self.f.fir.set_cur(None);
        }
        Ok(())
    }

    /// The constant `std::contracts::contract_violation` of `s` detected
    /// by `mode` (libstdc++'s layout: `{u16 version = 1, u16 kind, u16
    /// semantic, u16 mode, const char *comment, const void *src_loc, void
    /// *ext}`), and its `std::source_location::__impl` (`{const char
    /// *file, const char *function, u32 line, u32 column}`).
    fn contract_violation_object(&mut self, s: StmtId, mode: u16) -> R<Value> {
        if let Some(&g) = self.contract_objects.get(&(s, mode)) {
            return Ok(g);
        }
        let ast = self.ast;
        let site = ast.contract_sites.get(s).copied();
        let null = self.m.const_null(LTy::PTR);
        let mut ptrs = [null; 3];
        if let Some(site) = site {
            for (p, cv) in ptrs.iter_mut().zip([site.file, site.function, site.comment]) {
                *p = match ast.consts.scalar(cv) {
                    Some(sc) => self.scalar_const(sc, LTy::PTR)?,
                    None => return err(None, "contract site string is not a pointer"),
                };
            }
        } else {
            let empty = self.string_global(b"\0", 1, 1);
            ptrs = [Value::global(empty); 3];
        }
        let (line, column) = site.map_or((0, 0), |x| (x.line, x.column));
        let i16v = |cg: &mut Self, v: u16| cg.m.const_int(LTy::I16, i128::from(v));
        let i32v = |cg: &mut Self, v: u32| cg.m.const_int(LTy::I32, i128::from(v));
        let loc_ty = self.m.types.struct_literal(&[LTy::PTR, LTy::PTR, LTy::I32, LTy::I32], false);
        let loc_fields = [ptrs[0], ptrs[1], i32v(self, line), i32v(self, column)];
        let loc_init = self.m.const_struct(loc_ty, &loc_fields).map_err(|e| ir_err(&e))?;
        let loc = self.private_const(".contract.loc", loc_init, 8);
        let ty = self.m.types.struct_literal(
            &[LTy::I16, LTy::I16, LTy::I16, LTy::I16, LTy::PTR, LTy::PTR, LTy::PTR],
            false,
        );
        let fields = [
            i16v(self, 1),
            i16v(self, kind_value(ast.stmts.contract_kind(s))),
            i16v(self, semantic_value(ast.contract_semantic)),
            i16v(self, mode),
            ptrs[2],
            Value::global(loc),
            null,
        ];
        let init = self.m.const_struct(ty, &fields).map_err(|e| ir_err(&e))?;
        let g = Value::global(self.private_const(".contract", init, 8));
        self.contract_objects.insert((s, mode), g);
        Ok(g)
    }

    /// At the end of the translation unit: when a checked assertion was
    /// emitted (or the default handler is named) and the stdlib ships no
    /// handler, weak definitions of `handle_contract_violation` (calling
    /// the default) and `invoke_default_contract_violation_handler`
    /// (GCC's message on standard error). A user's strong definition
    /// replaces either at link time; one in this translation unit is
    /// kept.
    pub(crate) fn emit_contract_defaults(&mut self) -> R<()> {
        if !self.opts.contract_default_handler {
            return Ok(());
        }
        let undefined = |cg: &Self, name: &str| {
            cg.m.get_function(name).filter(|&f| cg.m.func(f).is_declaration())
        };
        let handler = undefined(self, HANDLER);
        let named_default = self.m.get_function(INVOKE_DEFAULT).is_some();
        if !(self.contract_checked && handler.is_some() || named_default) {
            return Ok(());
        }
        if self.m.get_function(INVOKE_DEFAULT).is_none() {
            self.rt_fn(INVOKE_DEFAULT, LTy::VOID, &[LTy::PTR], &[]);
        }
        if let Some(f) = undefined(self, INVOKE_DEFAULT) {
            self.define_default_handler(f)?;
        }
        if self.contract_checked
            && let Some(f) = handler
        {
            let inv = self.m.get_function(INVOKE_DEFAULT).expect("declared above");
            self.weak_definition(f, false);
            let mut b = IrBuilder::new(&mut self.m, f);
            let entry = b.append_block("");
            b.position_at_end(entry);
            b.call(inv, &[Value::arg(0)]).and_then(|_| b.ret_void()).map_err(|e| ir_err(&e))?;
        }
        Ok(())
    }

    /// Makes `f` a weak (replaceable, never inlined) definition with
    /// clang's definition attributes (and `nounwind` for a `noexcept` one).
    fn weak_definition(&mut self, f: kiln_llvm::FuncId, nounwind: bool) {
        let mut fa = vec![Attr::Flag(AttrKind::NoInline)];
        if nounwind {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
            self.nounwind.insert(f, true);
        }
        self.def_attrs(&mut fa);
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        let pa = self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
        let func = self.m.func_mut(f);
        func.attrs.func = fa;
        func.attrs.params = vec![pa];
        func.linkage = Linkage::Weak;
    }

    /// The body of the default `invoke_default_contract_violation_handler`
    /// `f`: `fflush(stdout)`, then `fprintf(stderr, DEFAULT_FORMAT, ...)`
    /// with the fields of the
    /// `contract_violation` (GCC 16.1's wording).
    fn define_default_handler(&mut self, f: kiln_llvm::FuncId) -> R<()> {
        self.weak_definition(f, true);
        let names = |cg: &mut Self, list: &[&str]| -> R<(Value, u64)> {
            let mut elems = Vec::with_capacity(list.len());
            for s in list {
                let mut bytes = s.as_bytes().to_vec();
                bytes.push(0);
                let n = bytes.len() as u64;
                elems.push(Value::global(cg.string_global(&bytes, 1, n)));
            }
            let init = cg.m.const_array(LTy::PTR, &elems).map_err(|e| ir_err(&e))?;
            Ok((Value::global(cg.private_const(".contract.names", init, 8)), list.len() as u64))
        };
        let kinds = names(self, &["unknown", "pre", "post", "assert"])?;
        let sems = names(self, &["unknown", "ignore", "observe", "enforce", "quick_enforce"])?;
        let modes = names(self, &["unknown", "predicate_false", "evaluation_exception"])?;
        let mut fmt = DEFAULT_FORMAT.as_bytes().to_vec();
        fmt.push(0);
        let n = fmt.len() as u64;
        let fmt = Value::global(self.string_global(&fmt, 1, n));
        let yes = Value::global(self.string_global(b"yes\0", 1, 4));
        let no = Value::global(self.string_global(b"no\0", 1, 3));
        let stream = |cg: &mut Self, name: &str| match cg.m.get_global(name) {
            Some(g) => g,
            None => {
                let g = cg.m.add_global(name, LTy::PTR);
                cg.m.global_mut(g).align = Some(8);
                g
            }
        };
        let stdout = stream(self, "stdout");
        let stderr = stream(self, "stderr");
        let fprintf_ty = self.m.types.function(LTy::I32, &[LTy::PTR, LTy::PTR], true);
        let fprintf = match self.m.get_function("fprintf") {
            Some(p) => p,
            None => self.m.add_function("fprintf", fprintf_ty).map_err(|e| ir_err(&e))?,
        };
        let fflush = self.rt_fn("fflush", LTy::I32, &[LTy::PTR], &[]);
        let fflush_ty = self.m.func(fflush).ty();
        let cv_ty = self.m.types.struct_literal(
            &[LTy::I16, LTy::I16, LTy::I16, LTy::I16, LTy::PTR, LTy::PTR, LTy::PTR],
            false,
        );
        let loc_ty = self.m.types.struct_literal(&[LTy::PTR, LTy::PTR, LTy::I32, LTy::I32], false);
        let three = self.m.const_int(LTy::I16, 3);
        let four = self.m.const_int(LTy::I16, 4);
        let zero = self.m.const_int(LTy::I64, 0);
        let mut b = IrBuilder::new(&mut self.m, f);
        let entry = b.append_block("");
        b.position_at_end(entry);
        let r = (|| {
            let v = Value::arg(0);
            let field = |b: &mut IrBuilder<'_>, i: u32, t: LTy, al: u64| {
                let p = b.struct_gep(cv_ty, v, i)?;
                b.load(t, p, al)
            };
            let kind = field(&mut b, 1, LTy::I16, 2)?;
            let sem = field(&mut b, 2, LTy::I16, 4)?;
            let mode = field(&mut b, 3, LTy::I16, 2)?;
            let comment = field(&mut b, 4, LTy::PTR, 8)?;
            let loc = field(&mut b, 5, LTy::PTR, 8)?;
            let lp = b.struct_gep(loc_ty, loc, 0)?;
            let file = b.load(LTy::PTR, lp, 8)?;
            let lp = b.struct_gep(loc_ty, loc, 1)?;
            let func = b.load(LTy::PTR, lp, 8)?;
            let lp = b.struct_gep(loc_ty, loc, 2)?;
            let line = b.load(LTy::I32, lp, 8)?;
            let name = |b: &mut IrBuilder<'_>, x: Value, (table, n): (Value, u64)| {
                let i = b.cast(kiln_llvm::CastOp::ZExt, x, LTy::I64)?;
                let lim = b.module().const_int(LTy::I64, i128::from(n));
                let ok = b.icmp(IntPredicate::Ult, i, lim)?;
                let i = b.select(ok, i, zero)?;
                let at = b.module().types.array(LTy::PTR, n);
                let p = b.gep(at, table, &[zero, i], GepFlags::INBOUNDS)?;
                b.load(LTy::PTR, p, 8)
            };
            let k = name(&mut b, kind, kinds)?;
            let s = name(&mut b, sem, sems)?;
            let m = name(&mut b, mode, modes)?;
            let e = b.icmp(IntPredicate::Eq, sem, three)?;
            let q = b.icmp(IntPredicate::Eq, sem, four)?;
            let t = b.binop(kiln_llvm::BinOp::Or, e, q)?;
            let t = b.select(t, yes, no)?;
            // libstdc++'s handler writes to `std::cerr`, which flushes the
            // synchronised `std::cout`, and so `stdout`, first.
            let out = b.load(LTy::PTR, Value::global(stdout), 8)?;
            b.call_ex(fflush_ty, Value::func(fflush), &[out], &CallOpts::default())?;
            let err = b.load(LTy::PTR, Value::global(stderr), 8)?;
            b.call_ex(
                fprintf_ty,
                Value::func(fprintf),
                &[err, fmt, func, file, line, comment, k, s, m, t],
                &CallOpts::default(),
            )?;
            b.ret_void()
        })();
        r.map(|_| ()).map_err(|e| ir_err(&e))
    }
}

fn ir_err(e: &kiln_llvm::IrError) -> crate::CodegenError {
    crate::CodegenError { loc: None, msg: format!("contract violation handler: {e:?}") }
}
