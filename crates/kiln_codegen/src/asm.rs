//! GNU inline assembly (clang `CodeGenFunction::EmitAsmStmt`,
//! `CGStmt.cpp`) and namespace-scope `asm` declarations
//! (`CodeGenModule::EmitTopLevelDecl`, `FileScopeAsmDecl`).

use kiln_abi::asm::{self as tasm, AsmOperandCounts, ConstraintInfo};
use kiln_ast::decl::VarStorage;
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::stmt::asm_bits;
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_llvm::{
    Attr, AttrKind, AttrList, AttrSetId, CallOpts, CastOp, InlineAsm, IntPredicate, MemoryEffects,
    ModRef, TypeId as LTy,
};

use crate::fir::{Blk, Fir, Op, V};
use crate::module::{Cg, R, err};

/// The register outputs of an asm call and where they are stored.
struct AsmOuts {
    call: V,
    result_tys: Vec<LTy>,
    trunc_tys: Vec<LTy>,
    dests: Vec<(crate::expr::LV, QualType)>,
    needs_cast: Vec<bool>,
    /// Condition-flag outputs (`=@cc<cond>`), whose value is 0 or 1.
    is_flag: Vec<bool>,
}

impl Cg<'_> {
    /// `__builtin_observable_checkpoint()` (P1494R5, [intro.abstract]/5;
    /// spec/codegen.md "Observable checkpoints"): an opaque barrier that
    /// emits no instruction, as GCC 16's does, written as `asm volatile(""
    /// ::: "memory")` would be:
    /// `call void asm sideeffect "", "~{memory}"()` (plus the target's
    /// implicit clobbers) marked `nounwind`. LLVM treats the call as
    /// possibly reading and writing all memory and, lacking `willreturn`,
    /// as possibly not returning, so neither a store before it nor the
    /// call itself is removed or moved past it on account of undefined
    /// behavior after it.
    pub(crate) fn emit_observable_checkpoint(&mut self) -> R<()> {
        let arch = self.target.triple.arch;
        let mut constraints = String::from("~{memory}");
        let machine = tasm::machine_clobbers(arch);
        if !machine.is_empty() {
            constraints.push(',');
            constraints.push_str(machine);
        }
        let fn_ty = self.m.types.function(LTy::VOID, &[], false);
        let ia = InlineAsm {
            fn_ty,
            asm: String::new().into(),
            constraints: constraints.into(),
            side_effect: true,
            align_stack: false,
            intel_dialect: false,
            can_throw: false,
        };
        let callee = self.m.inline_asm(ia).map_err(|e| crate::consts::ir_err(&e))?;
        let func = self.m.attr_set(&[Attr::Flag(AttrKind::NoUnwind)]);
        let call_opts =
            CallOpts { attrs: AttrList { func, ..AttrList::default() }, ..CallOpts::default() };
        self.emit_call_op(fn_ty, V::C(callee), Vec::new(), call_opts, LTy::VOID, None, true);
        Ok(())
    }

    /// clang `EmitAsmStmt` for a `GCCAsmStmt` (`asm goto` is a `callbr`).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn emit_asm(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let st = &ast.stmts;
        let loc = st.get(s).loc();
        let data = *st.asm_data(s);
        let bits = st.asm_bits(s);
        let goto = bits & asm_bits::GOTO != 0;
        let labels: Vec<DeclId> = ast.lists.iter::<DeclId>(data.labels).collect();
        let ops = st.asm_operands(s).to_vec();
        let nout = data.num_outputs as usize;
        let arch = self.target.triple.arch;
        let cstr = |l| String::from_utf8_lossy(ast.strings.bytes(l)).into_owned();
        let ident = |n: Option<kiln_source::symbol::Symbol>| {
            n.map_or_else(String::new, |n| ast.symbols().as_str(n).to_owned())
        };
        let mut names: Vec<String> = ops.iter().map(|o| ident(o.name)).collect();
        names.extend(labels.iter().map(|&l| ident(ast.decls.get(l).name().as_ident())));

        // The final template (`generateAsmString`).
        let simple = bits & asm_bits::BASIC != 0;
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut outs: Vec<ConstraintInfo> = Vec::new();
        for (i, o) in ops[..nout].iter().enumerate() {
            let mut info = ConstraintInfo::new(&cstr(o.constraint), &names[i]);
            tasm::validate_output_constraint(arch, &mut info);
            outs.push(info);
        }
        let mut ins: Vec<ConstraintInfo> = Vec::new();
        for (i, o) in ops[nout..].iter().enumerate() {
            let mut info = ConstraintInfo::new(&cstr(o.constraint), &names[nout + i]);
            tasm::validate_input_constraint(arch, &mut outs, &mut info);
            ins.push(info);
        }
        let counts = AsmOperandCounts {
            outputs: nout as u32,
            plus: outs.iter().filter(|o| o.is_read_write()).count() as u32,
            inputs: data.num_inputs,
            labels: labels.len() as u32,
            names: &name_refs,
        };
        let Ok(pieces) =
            tasm::analyze_asm_string(arch, ast.strings.bytes(data.string), simple, &counts)
        else {
            return err(Some(loc), "invalid asm template reached codegen");
        };
        let asm_text = String::from_utf8_lossy(&tasm::generate_asm_string(&pieces)).into_owned();

        let mut constraints = String::new();
        let mut arg_tys: Vec<LTy> = Vec::new();
        let mut elem_tys: Vec<Option<LTy>> = Vec::new();
        let mut args: Vec<V> = Vec::new();
        let mut result_tys: Vec<LTy> = Vec::new();
        let mut trunc_tys: Vec<LTy> = Vec::new();
        let mut dests = Vec::new();
        let mut needs_cast: Vec<bool> = Vec::new();
        let mut is_flag: Vec<bool> = Vec::new();
        let mut inout = String::new();
        let mut inout_args: Vec<(V, LTy, Option<LTy>)> = Vec::new();
        let mut out_constraints: Vec<String> = Vec::new();
        let (mut read_only, mut read_none) = (true, true);

        for (i, o) in ops[..nout].iter().enumerate() {
            let info = outs[i].clone();
            let c = cstr(o.constraint);
            let oc = tasm::simplify_constraint(arch, c.get(1..).unwrap_or(""), &outs);
            let pinned = self.reg_var(o.expr).is_some();
            let oc = self.var_constraint(oc, o.expr, info.early_clobber());
            out_constraints.push(oc.clone());
            let dest = self.emit_lvalue(o.expr)?;
            if !constraints.is_empty() {
                constraints.push(',');
            }
            let q = ast.exprs.get(o.expr).ty();
            if info.allows_memory() {
                arg_tys.push(LTy::PTR);
                elem_tys.push(Some(dest.addr.ty));
                args.push(dest.addr.v);
                constraints.push_str("=*");
                constraints.push_str(&oc);
                read_only = false;
                read_none = false;
            } else {
                constraints.push('=');
                constraints.push_str(&oc);
                let mt = self.mty(q);
                let cast = info.allows_register() && self.m.types.is_aggregate(mt);
                trunc_tys.push(mt);
                needs_cast.push(cast);
                is_flag.push(oc.starts_with("{@cc"));
                let mut rt = if cast {
                    let (size, _) = self.size_align(q);
                    if size == 0 {
                        return err(Some(loc), "output size should not be zero");
                    }
                    self.m.types.int((size * 8) as u32)
                } else {
                    mt
                };
                if info.has_matching_input()
                    && let Some(k) = ins.iter().position(|n| n.tied_operand() == Some(i as u32))
                {
                    let it = ast.exprs.get(ops[nout + k].expr).ty();
                    if self.size_align(q).0 < self.size_align(it).0 {
                        rt = self.lty(it);
                    }
                }
                result_tys.push(rt);
                dests.push((dest, q));
            }
            if info.is_read_write() {
                inout.push(',');
                let (arg, elem) = self.asm_input_lvalue(&info, dest, q, &mut inout);
                let at = self.f.fir.ty(&self.m, arg);
                // Only an early-clobber physical register is tied.
                if info.allows_register() && (!pinned || info.early_clobber()) {
                    inout.push_str(&i.to_string());
                } else {
                    inout.push_str(&oc);
                }
                inout_args.push((arg, at, elem));
            }
        }

        for (k, o) in ops[nout..].iter().enumerate() {
            let info = ins[k].clone();
            if info.allows_memory() {
                read_none = false;
            }
            if !constraints.is_empty() {
                constraints.push(',');
            }
            let ic = tasm::simplify_constraint(arch, &cstr(o.constraint), &outs);
            let ic = self.var_constraint(ic, o.expr, false);
            let (mut arg, elem) = self.asm_input(&info, o.expr, &mut constraints)?;
            if let Some(out) = info.tied_operand() {
                let oq = ast.exprs.get(ops[out as usize].expr).ty();
                let iq = ast.exprs.get(o.expr).ty();
                if self.size_align(oq).0 > self.size_align(iq).0 {
                    let intptr = self.m.types.int(64);
                    if self.m.types.is_ptr(self.f.fir.ty(&self.m, arg)) {
                        arg = self.cast(CastOp::PtrToInt, arg, intptr);
                    }
                    let ot = self.lty(oq);
                    if self.m.types.is_int(ot) {
                        arg = self.cast(CastOp::ZExt, arg, ot);
                    } else if self.m.types.is_ptr(ot) {
                        arg = self.cast(CastOp::ZExt, arg, intptr);
                    } else if self.m.types.is_float(ot) {
                        arg = self.cast(CastOp::FPExt, arg, ot);
                    }
                }
            }
            arg_tys.push(self.f.fir.ty(&self.m, arg));
            elem_tys.push(elem);
            args.push(arg);
            constraints.push_str(&ic);
        }
        for (a, t, e) in inout_args {
            arg_tys.push(t);
            elem_tys.push(e);
            args.push(a);
        }
        constraints.push_str(&inout);

        // `asm goto` labels: one `!i` each (clang `getJumpDestForLabel`;
        // no cleanups run on these edges).
        let mut transfer = Vec::with_capacity(labels.len());
        for &l in &labels {
            transfer.push(self.label_dest(l).blk);
            if !constraints.is_empty() {
                constraints.push(',');
            }
            constraints.push_str("!i");
        }

        let mut can_throw = false;
        for l in ast.lists.iter::<kiln_ast::StringLitId>(data.clobbers) {
            let raw = cstr(l);
            let clobber = match raw.as_str() {
                "memory" => {
                    read_only = false;
                    read_none = false;
                    raw
                }
                "unwind" => {
                    can_throw = true;
                    continue;
                }
                "cc" => raw,
                _ => tasm::normalized_register_name(arch, &raw),
            };
            if !constraints.is_empty() {
                constraints.push(',');
            }
            constraints.push_str("~{");
            constraints.push_str(&clobber);
            constraints.push('}');
        }
        let machine = tasm::machine_clobbers(arch);
        if !machine.is_empty() {
            if !constraints.is_empty() {
                constraints.push(',');
            }
            constraints.push_str(machine);
        }

        let ret = match result_tys.len() {
            0 => LTy::VOID,
            1 => result_tys[0],
            _ => self.m.types.struct_literal(&result_tys, false),
        };
        let fn_ty = self.m.types.function(ret, &arg_tys, false);
        let side_effect = bits & asm_bits::VOLATILE != 0 || nout == 0;
        let ia = InlineAsm {
            fn_ty,
            asm: asm_text.into(),
            constraints: constraints.into(),
            side_effect,
            align_stack: false,
            intel_dialect: false,
            can_throw,
        };
        let callee = self.m.inline_asm(ia).map_err(|e| crate::consts::ir_err(&e))?;

        // clang `UpdateAsmCallInst`.
        let mut fattrs = Vec::new();
        if !can_throw || goto {
            fattrs.push(Attr::Flag(AttrKind::NoUnwind));
        }
        if !side_effect {
            if read_none {
                fattrs.push(Attr::Memory(MemoryEffects::all(ModRef::None)));
            } else if read_only {
                fattrs.push(Attr::Memory(MemoryEffects::all(ModRef::Read)));
            }
        }
        let func = self.m.attr_set(&fattrs);
        let params: Vec<AttrSetId> = elem_tys
            .iter()
            .map(|e| match e {
                Some(t) => self.m.attr_set(&[Attr::ElementType(*t)]),
                None => self.m.attr_set(&[]),
            })
            .collect();
        let call_opts = CallOpts {
            attrs: AttrList { func, params, ..AttrList::default() },
            ..CallOpts::default()
        };
        // With outputs, each indirect destination gets its own block that
        // stores them (clang splits every indirect edge).
        let split: Vec<Blk> = if goto && !result_tys.is_empty() {
            transfer.iter().map(|_| self.f.fir.create_block()).collect()
        } else {
            Vec::new()
        };
        let call = if goto {
            let normal = self.f.fir.create_block();
            let indirect = if split.is_empty() { transfer.clone() } else { split.clone() };
            let op = Op::CallBr {
                fn_ty,
                callee: V::C(callee),
                args,
                opts: Box::new(call_opts),
                normal,
                indirect,
            };
            let call = self.f.fir.push(op, ret);
            self.f.calls.push((Fir::last_inst(call), None, true));
            self.emit_block(normal, false);
            call
        } else {
            // An `unwind` clobber may throw: an `invoke` in an EH scope.
            self.emit_call_op(fn_ty, V::C(callee), args, call_opts, ret, None, !can_throw)
        };
        let inst = Fir::last_inst(call);
        let mut locs = Vec::new();
        for &raw in ast.lists.get(data.srclocs) {
            let i64t = self.m.types.int(64);
            let c = self.m.const_int(i64t, i128::from(raw));
            locs.push(Some(self.m.md_const(c).map_err(|e| crate::consts::ir_err(&e))?));
        }
        let md = self.m.md_tuple(&locs);
        let kind = self.m.md_kind("srcloc");
        self.f.fir.add_md(inst, kind, md);

        let out = AsmOuts { call, result_tys, trunc_tys, dests, needs_cast, is_flag };
        self.asm_stores(&out);
        let fallthrough = self.f.fir.cur();
        for (k, b) in split.into_iter().enumerate() {
            self.f.fir.set_cur(Some(b));
            self.asm_stores(&out);
            self.f.fir.push(Op::Br { dest: transfer[k] }, LTy::VOID);
            self.f.fir.place_at_end(b);
        }
        self.f.fir.set_cur(fallthrough);
        Ok(())
    }

    /// clang `UpdateAsmCallInst`'s result extraction then `EmitAsmStores`,
    /// at the insertion point.
    /// clang `AddVariableConstraints`: an operand naming a `register`
    /// variable with an asm label (`register long r asm("x1")`) is pinned
    /// to that register, `{x1}` (`&{x1}` when early-clobber).
    fn var_constraint(&self, c: String, e: ExprId, early_clobber: bool) -> String {
        match self.reg_var(e) {
            Some(reg) => format!("{}{{{reg}}}", if early_clobber { "&" } else { "" }),
            None => c,
        }
    }

    /// The normalized register of a `register` variable with an asm label
    /// that `e` names (after `IgnoreParenNoopCasts`).
    fn reg_var(&self, e: ExprId) -> Option<String> {
        let ast = self.ast;
        // `IgnoreParenNoopCasts`: parentheses and identity casts.
        let mut e = e;
        loop {
            let n = ast.exprs.get(e);
            if n.kind() == ExprKind::ParenExpr
                || (n.kind().is_cast()
                    && matches!(ast.exprs.cast_kind(e), CastKind::LValueToRValue | CastKind::NoOp))
            {
                e = ast.exprs.operand(e);
            } else {
                break;
            }
        }
        if ast.exprs.get(e).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = ast.exprs.decl(e);
        if !ast.decls.var(d).is_some_and(|v| v.spec.storage() == VarStorage::Register) {
            return None;
        }
        let reg = kiln_abi::mangle::asm_label(ast, d)?;
        Some(tasm::normalized_register_name(self.target.triple.arch, &reg))
    }

    fn asm_stores(&mut self, o: &AsmOuts) {
        let (call, result_tys, trunc_tys) = (o.call, &o.result_tys, &o.trunc_tys);
        let results: Vec<V> = if result_tys.len() == 1 {
            vec![call]
        } else {
            (0..result_tys.len())
                .map(|i| {
                    self.f.fir.push(Op::ExtractValue { agg: call, idx: i as u32 }, result_tys[i])
                })
                .collect()
        };
        for (i, mut v) in results.into_iter().enumerate() {
            let (rt, tt) = (result_tys[i], trunc_tys[i]);
            if o.is_flag[i] {
                // clang: a flag output is 0 or 1; tell the optimizer.
                let two = V::C(self.m.const_int(rt, 2));
                let c = self.f.fir.push(Op::ICmp { p: IntPredicate::Ult, l: v, r: two }, LTy::I1);
                let f = kiln_llvm::intrinsics::assume(&mut self.m);
                self.intrinsic_call(f, vec![c]);
            }
            if rt != tt {
                let tys = &self.m.types;
                if tys.is_float(tt) {
                    v = self.cast(CastOp::FPTrunc, v, tt);
                } else if tys.is_ptr(tt) && tys.is_int(rt) {
                    let i64t = self.m.types.int(64);
                    v = self.int_cast(v, i64t, false);
                    v = self.cast(CastOp::IntToPtr, v, tt);
                } else if tys.is_ptr(rt) && tys.is_int(tt) {
                    let i64t = self.m.types.int(64);
                    v = self.cast(CastOp::PtrToInt, v, i64t);
                    v = self.int_cast(v, tt, false);
                } else if tys.is_int(rt) && tys.is_int(tt) {
                    v = self.int_cast(v, tt, false);
                }
            }
            let (mut dest, q) = o.dests[i];
            if o.needs_cast[i] {
                dest.addr.ty = rt;
                let vol = self.ast.types.effective_quals(q).is_volatile();
                self.store(v, dest.addr.v, dest.addr.align, vol);
                continue;
            }
            self.store_lv(v, dest, false);
        }
    }

    /// clang `EmitAsmInput`.
    fn asm_input(
        &mut self,
        info: &ConstraintInfo,
        e: ExprId,
        cs: &mut String,
    ) -> R<(V, Option<LTy>)> {
        let q = self.ast.exprs.get(e).ty();
        let scalar = !self.is_aggregate(q);
        if (info.allows_register() || !info.allows_memory()) && scalar && !self.is_glvalue(e) {
            return Ok((self.emit_scalar(e)?, None));
        }
        if (info.allows_register() || !info.allows_memory()) && scalar {
            let lv = self.emit_lvalue(e)?;
            return Ok((self.load_lv(lv), None));
        }
        let lv = self.emit_lvalue(e)?;
        Ok(self.asm_input_lvalue(info, lv, q, cs))
    }

    /// clang `EmitAsmInputLValue`.
    fn asm_input_lvalue(
        &mut self,
        info: &ConstraintInfo,
        lv: crate::expr::LV,
        q: QualType,
        cs: &mut String,
    ) -> (V, Option<LTy>) {
        if info.allows_register() || !info.allows_memory() {
            if !self.is_aggregate(q) {
                return (self.load_lv(lv), None);
            }
            let (size, _) = self.size_align(q);
            let bits = size * 8;
            if bits <= 64 && bits.is_power_of_two() {
                let it = self.m.types.int(bits as u32);
                let vol = self.ast.types.effective_quals(q).is_volatile();
                return (self.load(it, lv.addr.v, lv.addr.align, vol), None);
            }
        }
        cs.push('*');
        (lv.addr.v, Some(lv.addr.ty))
    }

    /// A namespace-scope `asm("...")` (clang appends it to the module's
    /// inline assembly).
    pub(crate) fn emit_file_scope_asm(&mut self, d: DeclId) {
        let Some(e) = self.ast.file_scope_asm_string(d) else { return };
        let lit = self.ast.exprs.string_lit(e);
        let text = String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned();
        self.m.module_asm.push(text);
    }
}
