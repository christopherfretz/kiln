//! GNU inline assembly (clang `CodeGenFunction::EmitAsmStmt`,
//! `CGStmt.cpp`) and namespace-scope `asm` declarations
//! (`CodeGenModule::EmitTopLevelDecl`, `FileScopeAsmDecl`).

use kiln_abi::asm::{self as tasm, AsmOperandCounts, ConstraintInfo};
use kiln_ast::stmt::asm_bits;
use kiln_ast::{QualType, StmtId};
use kiln_llvm::{
    Attr, AttrKind, AttrList, AttrSetId, CallOpts, CastOp, InlineAsm, MemoryEffects, ModRef,
    TypeId as LTy,
};

use crate::fir::{Fir, Op, V};
use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// clang `EmitAsmStmt` for a `GCCAsmStmt` without `goto` labels.
    pub(crate) fn emit_asm(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let st = &ast.stmts;
        let loc = st.get(s).loc();
        let data = *st.asm_data(s);
        let bits = st.asm_bits(s);
        if bits & asm_bits::GOTO != 0 {
            return err(Some(loc), "'asm goto' is not supported yet");
        }
        let ops = st.asm_operands(s).to_vec();
        let nout = data.num_outputs as usize;
        let arch = self.target.triple.arch;
        let cstr = |l| String::from_utf8_lossy(ast.strings.bytes(l)).into_owned();
        let names: Vec<String> = ops
            .iter()
            .map(|o| o.name.map_or_else(String::new, |n| ast.symbols().as_str(n).to_owned()))
            .collect();

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
            labels: 0,
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
        let mut inout = String::new();
        let mut inout_args: Vec<(V, LTy, Option<LTy>)> = Vec::new();
        let mut out_constraints: Vec<String> = Vec::new();
        let (mut read_only, mut read_none) = (true, true);

        for (i, o) in ops[..nout].iter().enumerate() {
            let info = outs[i].clone();
            let c = cstr(o.constraint);
            let oc = tasm::simplify_constraint(arch, c.get(1..).unwrap_or(""), &outs);
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
                if info.allows_register() {
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
        if !can_throw {
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
        let call = self
            .f
            .fir
            .push(Op::Call { fn_ty, callee: V::C(callee), args, opts: Box::new(call_opts) }, ret);
        let inst = Fir::last_inst(call);
        self.f.calls.push((inst, None, !can_throw));
        let mut locs = Vec::new();
        for &raw in ast.lists.get(data.srclocs) {
            let i64t = self.m.types.int(64);
            let c = self.m.const_int(i64t, i128::from(raw));
            locs.push(Some(self.m.md_const(c).map_err(|e| crate::consts::ir_err(&e))?));
        }
        let md = self.m.md_tuple(&locs);
        let kind = self.m.md_kind("srcloc");
        self.f.fir.add_md(inst, kind, md);

        let results: Vec<V> = if result_tys.len() == 1 {
            vec![call]
        } else {
            (0..result_tys.len())
                .map(|i| {
                    self.f.fir.push(Op::ExtractValue { agg: call, idx: i as u32 }, result_tys[i])
                })
                .collect()
        };
        // clang `EmitAsmStores`.
        for (i, mut v) in results.into_iter().enumerate() {
            let (rt, tt) = (result_tys[i], trunc_tys[i]);
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
            let (mut dest, q) = dests[i];
            if needs_cast[i] {
                dest.addr.ty = rt;
                let vol = self.ast.types.effective_quals(q).is_volatile();
                self.store(v, dest.addr.v, dest.addr.align, vol);
                continue;
            }
            self.store_lv(v, dest, false);
        }
        Ok(())
    }

    /// clang `EmitAsmInput`.
    fn asm_input(
        &mut self,
        info: &ConstraintInfo,
        e: kiln_ast::ExprId,
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
    pub(crate) fn emit_file_scope_asm(&mut self, d: kiln_ast::DeclId) {
        let Some(e) = self.ast.file_scope_asm_string(d) else { return };
        let lit = self.ast.exprs.string_lit(e);
        let text = String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned();
        self.m.module_asm.push(text);
    }
}
