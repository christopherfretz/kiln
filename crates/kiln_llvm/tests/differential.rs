//! Differential tests against clang (`spec/llvm.md` §9.2).
//!
//! For every C program in `tests/cprogs/`, the test hand-builds the module
//! `clang-19 -S -emit-llvm -O0 --target=aarch64-unknown-linux-gnu` produces
//! for it, then compares both texts after normalizing each with
//! `opt-19 -S -passes=verify`. Our own text must additionally be a fixed
//! point of `opt -S` (§7). The builders mirror clang's `-O0` lowering
//! instruction for instruction, which is exactly what `kiln_codegen` will
//! have to do.

#![allow(clippy::many_single_char_names, clippy::similar_names, clippy::too_many_lines)]

mod common;

use kiln_llvm::intrinsics::{self, OverflowOp};
use kiln_llvm::*;

const I1: TypeId = TypeId::I1;
const I8: TypeId = TypeId::I8;
const I16: TypeId = TypeId::I16;
const I32: TypeId = TypeId::I32;
const I64: TypeId = TypeId::I64;
const F32: TypeId = TypeId::FLOAT;
const F64: TypeId = TypeId::DOUBLE;
const PTR: TypeId = TypeId::PTR;
const VOID: TypeId = TypeId::VOID;

/// The programs under `tests/cprogs/`, each with a test below.
const PROGRAMS: &[&str] = &[
    "add", "atomics", "bits", "builtins", "calls", "floats", "globals", "labels", "logic", "loop",
    "pointers", "structs", "switch", "tls", "varargs", "vla",
];

/// Every C program has a differential test, and vice versa.
#[test]
fn all_programs_covered() {
    let mut found: Vec<String> = std::fs::read_dir(common::cprogs_dir())
        .expect("read tests/cprogs")
        .filter_map(|e| {
            let p = e.expect("dir entry").path();
            (p.extension()? == "c").then(|| p.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    found.sort();
    assert_eq!(found, PROGRAMS);
}

/// Print `m`, check it with the verifier and the LLVM tools, and compare it
/// with clang's output for `tests/cprogs/<prog>.c`, both through `opt -S`.
fn differential(prog: &str, m: &Module) {
    m.verify().expect("module verifies");
    let text = m.to_text();
    if !common::llvm_accepts(&text) {
        return;
    }
    common::assert_fixed_point(&text);
    let Some(reference) = common::clang_ir(prog) else { return };
    let (Some(ours), Some(theirs)) = (common::canonical(&text), common::canonical(&reference))
    else {
        return;
    };
    common::assert_same_text(
        &format!("{prog}.c: kiln_llvm vs clang"),
        &common::strip_comments(&ours),
        &common::strip_comments(&theirs),
    );
}

// ---- clang -O0 conventions ----------------------------------------------

/// A module with the header and module flags clang emits for aarch64.
fn clang_module(prog: &str) -> Module {
    let mut m = Module::with_target(&format!("{prog}.c"), AARCH64_LINUX_GNU);
    m.add_module_flag_i32(1, "wchar_size", 4);
    m.add_module_flag_i32(8, "PIC Level", 2);
    m.add_module_flag_i32(7, "PIE Level", 2);
    m.add_module_flag_i32(7, "uwtable", 2);
    m.add_module_flag_i32(7, "frame-pointer", 1);
    m
}

/// The string attributes clang 19 puts on every function for this target.
fn target_attrs() -> Vec<Attr> {
    vec![
        Attr::string("frame-pointer", "non-leaf"),
        Attr::string("no-trapping-math", "true"),
        Attr::string("stack-protector-buffer-size", "8"),
        Attr::string("target-cpu", "generic"),
        Attr::string("target-features", "+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"),
    ]
}

fn noundef(m: &mut Module) -> AttrSetId {
    m.attr_set(&[Attr::Flag(AttrKind::NoUndef)])
}

/// A function with `noundef` parameters and the given function attributes.
fn function(
    m: &mut Module,
    name: &str,
    ret: TypeId,
    params: &[TypeId],
    varargs: bool,
    fn_attrs: &[Attr],
) -> FuncId {
    let fty = m.types.function(ret, params, varargs);
    let f = m.add_function(name, fty).expect("function type");
    let fa = m.attr_set(fn_attrs);
    let nu = noundef(m);
    let func = m.func_mut(f);
    func.dso_local = true;
    func.attrs.func = fa;
    for i in 0..params.len() {
        func.attrs.set_param(i, nu);
    }
    f
}

/// A `define dso_local` with clang's `-O0` attributes (`#0`). The body is
/// added later with [`at_entry`].
fn define(m: &mut Module, name: &str, ret: TypeId, params: &[TypeId], varargs: bool) -> FuncId {
    let mut attrs = vec![
        Attr::Flag(AttrKind::NoInline),
        Attr::Flag(AttrKind::NoUnwind),
        Attr::Flag(AttrKind::OptimizeNone),
        Attr::UwTable(UwTableKind::Async),
    ];
    attrs.extend(target_attrs());
    function(m, name, ret, params, varargs, &attrs)
}

/// An external declaration (`declare i32 @printf(ptr noundef, ...)`).
fn declare(m: &mut Module, name: &str, ret: TypeId, params: &[TypeId], varargs: bool) -> FuncId {
    let f = function(m, name, ret, params, varargs, &target_attrs());
    m.func_mut(f).dso_local = false;
    f
}

/// A builder positioned in a new entry block of `f`.
fn at_entry(m: &mut Module, f: FuncId) -> IrBuilder<'_> {
    let mut b = IrBuilder::new(m, f);
    let e = b.append_block("");
    b.position_at_end(e);
    b
}

fn arg(b: &IrBuilder<'_>, n: u32) -> Value {
    b.func_ref().arg(n)
}

fn int(b: &mut IrBuilder<'_>, ty: TypeId, v: i128) -> Value {
    b.module().const_int(ty, v)
}

/// A call whose arguments are all `noundef`, as clang emits for calls to
/// ordinary C functions.
fn call_noundef(
    b: &mut IrBuilder<'_>,
    fn_ty: TypeId,
    callee: Value,
    args: &[Value],
) -> IrResult<Value> {
    let nu = noundef(b.module());
    let mut o = CallOpts::default();
    for i in 0..args.len() {
        o.attrs.set_param(i, nu);
    }
    b.call_ex(fn_ty, callee, args, &o)
}

fn call_fn(b: &mut IrBuilder<'_>, f: FuncId, args: &[Value]) -> IrResult<Value> {
    let ty = b.module_ref().func(f).ty();
    call_noundef(b, ty, Value::func(f), args)
}

fn ordered(align: u64, ordering: Ordering) -> MemOpts {
    MemOpts { ordering, ..MemOpts::aligned(align) }
}

/// A private `unnamed_addr constant` C string (`@.str`).
fn string_literal(m: &mut Module, s: &[u8]) -> GlobalId {
    let init = m.const_cstring(s);
    let ty = m.types.array(I8, s.len() as u64 + 1);
    let g = m.add_global(".str", ty);
    let gv = m.global_mut(g);
    gv.is_constant = true;
    gv.linkage = Linkage::Private;
    gv.unnamed_addr = UnnamedAddr::Global;
    gv.align = Some(1);
    m.set_initializer(g, Some(init)).expect("initializer type");
    g
}

/// A `dso_local global` with an initializer.
fn global(m: &mut Module, name: &str, ty: TypeId, init: Value, align: u64) -> GlobalId {
    let g = m.add_global(name, ty);
    let gv = m.global_mut(g);
    gv.dso_local = true;
    gv.align = Some(align);
    m.set_initializer(g, Some(init)).expect("initializer type");
    g
}

/// `!llvm.loop !N` with `llvm.loop.mustprogress` on a loop back edge.
fn mark_loop(b: &mut IrBuilder<'_>, br: Value) -> IrResult<()> {
    let m = b.module();
    let s = m.md_string(b"llvm.loop.mustprogress");
    let mp = m.md_tuple(&[Some(s)]);
    let lp = m.md_distinct_tuple(&[None, Some(mp)]);
    m.md_set_operand(lp, 0, Some(lp))?;
    let kind = m.md_kind("llvm.loop");
    b.func().add_inst_metadata(br.as_inst().expect("an instruction"), kind, lp);
    Ok(())
}

// ---- the programs -------------------------------------------------------

fn build_add() -> IrResult<Module> {
    let mut m = clang_module("add");
    let f = define(&mut m, "add", I32, &[I32, I32], false);
    let mut b = at_entry(&mut m, f);
    let pa = b.alloca(I32, 4)?;
    let pb = b.alloca(I32, 4)?;
    b.store(arg(&b, 0), pa, 4)?;
    b.store(arg(&b, 1), pb, 4)?;
    let x = b.load(I32, pa, 4)?;
    let y = b.load(I32, pb, 4)?;
    let s = b.add_nsw(x, y)?;
    b.ret(Some(s))?;
    Ok(m)
}

/// §9.2: arguments spilled to allocas, `add nsw`.
#[test]
fn add() {
    differential("add", &build_add().unwrap());
}

fn build_loop() -> IrResult<Module> {
    let mut m = clang_module("loop");
    let f = define(&mut m, "sum", I32, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let (cond, body, inc, end) =
        (b.append_block(""), b.append_block(""), b.append_block(""), b.append_block(""));
    let pn = b.alloca(I32, 4)?;
    let ps = b.alloca(I32, 4)?;
    let pi = b.alloca(I32, 4)?;
    b.store(arg(&b, 0), pn, 4)?;
    let zero = int(&mut b, I32, 0);
    b.store(zero, ps, 4)?;
    b.store(zero, pi, 4)?;
    b.br(cond)?;
    b.position_at_end(cond);
    let i = b.load(I32, pi, 4)?;
    let n = b.load(I32, pn, 4)?;
    let c = b.icmp(IntPredicate::Slt, i, n)?;
    b.cond_br(c, body, end)?;
    b.position_at_end(body);
    let i = b.load(I32, pi, 4)?;
    let s = b.load(I32, ps, 4)?;
    let t = b.add_nsw(s, i)?;
    b.store(t, ps, 4)?;
    b.br(inc)?;
    b.position_at_end(inc);
    let i = b.load(I32, pi, 4)?;
    let one = int(&mut b, I32, 1);
    let i1 = b.add_nsw(i, one)?;
    b.store(i1, pi, 4)?;
    let back = b.br(cond)?;
    mark_loop(&mut b, back)?;
    b.position_at_end(end);
    let s = b.load(I32, ps, 4)?;
    b.ret(Some(s))?;
    Ok(m)
}

/// §9.2: unnamed blocks, `icmp`, `br`, and `!llvm.loop` metadata.
#[test]
fn loop_() {
    differential("loop", &build_loop().unwrap());
}

fn build_globals() -> IrResult<Module> {
    let mut m = clang_module("globals");
    let three = m.const_int(I32, 3);
    let counter = global(&mut m, "counter", I32, three, 4);
    let s = string_literal(&mut m, b"hi\n");
    global(&mut m, "greeting", PTR, Value::global(s), 8);
    let elems: Vec<Value> = [1, 2, 3, 0].iter().map(|&v| m.const_int(I32, v)).collect();
    let table_init = m.const_array(I32, &elems)?;
    let table_ty = m.types.array(I32, 4);
    let table = global(&mut m, "table", table_ty, table_init, 4);
    let four = m.const_int(I64, 4);
    let second = m.const_gep(I8, Value::global(table), &[four], GepFlags::NONE, None)?;
    global(&mut m, "second", PTR, second, 8);
    let half = m.const_float(F64, 0.5);
    let ratio = global(&mut m, "ratio", F64, half, 8);
    let gv = m.global_mut(ratio);
    gv.linkage = Linkage::Internal;
    gv.dso_local = false;
    let ext = m.add_global("ext", I32);
    m.global_mut(ext).align = Some(4);

    let f = define(&mut m, "get", F64, &[], false);
    let mut b = at_entry(&mut m, f);
    let r = b.load(F64, Value::global(ratio), 8)?;
    let e = b.load(I32, Value::global(ext), 4)?;
    let e = b.cast(CastOp::SIToFP, e, F64)?;
    let t = b.binop(BinOp::FAdd, r, e)?;
    let c = b.load(I32, Value::global(counter), 4)?;
    let c = b.cast(CastOp::SIToFP, c, F64)?;
    let t = b.binop(BinOp::FAdd, t, c)?;
    b.ret(Some(t))?;
    Ok(m)
}

/// §2.3/§3.2: initialized, string, array, internal and external globals,
/// and a constant `getelementptr` initializer.
#[test]
fn globals() {
    differential("globals", &build_globals().unwrap());
}

fn build_structs() -> IrResult<Module> {
    let mut m = clang_module("structs");
    let p = m.types.named_struct("struct.P");
    m.types.set_body(p, &[I32, I64], false)?;
    let big = m.types.named_struct("struct.Big");
    m.types.set_body(big, &[I64, I64, I64], false)?;
    let (one, two) = (m.const_int(I32, 1), m.const_int(I64, 2));
    let init = m.const_struct(p, &[one, two])?;
    global(&mut m, "g", p, init, 8);

    let sum = define(&mut m, "sum", I64, &[PTR], false);
    let mut b = at_entry(&mut m, sum);
    let pp = b.alloca(PTR, 8)?;
    b.store(arg(&b, 0), pp, 8)?;
    let q = b.load(PTR, pp, 8)?;
    let fx = b.struct_gep(p, q, 0)?;
    let x = b.load(I32, fx, 8)?;
    let x = b.cast(CastOp::SExt, x, I64)?;
    let q = b.load(PTR, pp, 8)?;
    let fy = b.struct_gep(p, q, 1)?;
    let y = b.load(I64, fy, 8)?;
    let s = b.add_nsw(x, y)?;
    b.ret(Some(s))?;

    let make = define(&mut m, "make", VOID, &[PTR, PTR], false);
    let sret = m.attr_set(&[
        Attr::Flag(AttrKind::DeadOnUnwind),
        Attr::Flag(AttrKind::NoAlias),
        Attr::Flag(AttrKind::Writable),
        Attr::SRet(big),
        Attr::Align(8),
    ]);
    m.func_mut(make).attrs.set_param(0, sret);
    let memcpy = intrinsics::memcpy(&mut m, I64);
    let align8 = m.attr_set(&[Attr::Align(8)]);
    let mut b = at_entry(&mut m, make);
    let pin = b.alloca(PTR, 8)?;
    b.store(arg(&b, 1), pin, 8)?;
    let mut o = CallOpts::default();
    o.attrs.set_param(0, align8);
    o.attrs.set_param(1, align8);
    let (len, no) = (int(&mut b, I64, 24), b.module().const_bool(false));
    let ty = b.module_ref().func(memcpy).ty();
    b.call_ex(ty, Value::func(memcpy), &[arg(&b, 0), arg(&b, 1), len, no], &o)?;
    let fa = b.struct_gep(big, arg(&b, 0), 0)?;
    let one = int(&mut b, I64, 1);
    b.store(one, fa, 8)?;
    b.ret_void()?;
    Ok(m)
}

/// §2.1/§5: named struct types, struct GEPs, `sret` and an aggregate copy
/// through `llvm.memcpy`.
#[test]
fn structs() {
    differential("structs", &build_structs().unwrap());
}

fn build_switch() -> IrResult<Module> {
    let mut m = clang_module("switch");
    let f = define(&mut m, "classify", I32, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let [c1, c23, cm5, dflt, brk, ret] = [(); 6].map(|()| b.append_block(""));
    let pr = b.alloca(I32, 4)?;
    let pc = b.alloca(I32, 4)?;
    b.store(arg(&b, 0), pc, 4)?;
    let c = b.load(I32, pc, 4)?;
    let cases = [(1, c1), (2, c23), (3, c23), (-5, cm5)].map(|(v, bb)| (int(&mut b, I32, v), bb));
    b.switch(c, dflt, &cases)?;
    for (bb, slot, v, next) in
        [(c1, pr, 10, ret), (c23, pc, 7, brk), (cm5, pr, 1, ret), (dflt, pc, 0, brk)]
    {
        b.position_at_end(bb);
        let v = int(&mut b, I32, v);
        b.store(v, slot, 4)?;
        b.br(next)?;
    }
    b.position_at_end(brk);
    let c = b.load(I32, pc, 4)?;
    b.store(c, pr, 4)?;
    b.br(ret)?;
    b.position_at_end(ret);
    let r = b.load(I32, pr, 4)?;
    b.ret(Some(r))?;
    Ok(m)
}

/// §4: `switch` with a shared destination and a negative case.
#[test]
fn switch() {
    differential("switch", &build_switch().unwrap());
}

fn build_varargs() -> IrResult<Module> {
    let mut m = clang_module("varargs");
    let va = m.types.named_struct("struct.__va_list");
    m.types.set_body(va, &[PTR, PTR, PTR, I32, I32], false)?;
    let fmt = string_literal(&mut m, b"%d %f\n");

    let total = define(&mut m, "total", I32, &[I32], true);
    let va_start = intrinsics::va_start(&mut m);
    let va_end = intrinsics::va_end(&mut m);
    let mut b = at_entry(&mut m, total);
    let [reg, in_reg, on_stack, done] = [(); 4].map(|()| b.append_block(""));
    let pn = b.alloca(I32, 4)?;
    let ap = b.alloca(va, 8)?;
    let pt = b.alloca(I32, 4)?;
    b.store(arg(&b, 0), pn, 4)?;
    b.call(va_start, &[ap])?;
    let gr_offs_p = b.struct_gep(va, ap, 3)?;
    let gr_offs = b.load(I32, gr_offs_p, 8)?;
    let zero = int(&mut b, I32, 0);
    let c = b.icmp(IntPredicate::Sge, gr_offs, zero)?;
    b.cond_br(c, on_stack, reg)?;

    b.position_at_end(reg);
    let eight = int(&mut b, I32, 8);
    let new_offs = b.add(gr_offs, eight)?;
    b.store(new_offs, gr_offs_p, 8)?;
    let c = b.icmp(IntPredicate::Sle, new_offs, zero)?;
    b.cond_br(c, in_reg, on_stack)?;

    b.position_at_end(in_reg);
    let gr_top_p = b.struct_gep(va, ap, 1)?;
    let gr_top = b.load(PTR, gr_top_p, 8)?;
    let reg_addr = b.gep(I8, gr_top, &[gr_offs], GepFlags::INBOUNDS)?;
    b.br(done)?;

    b.position_at_end(on_stack);
    let stack_p = b.struct_gep(va, ap, 0)?;
    let stack = b.load(PTR, stack_p, 8)?;
    let eight64 = int(&mut b, I64, 8);
    let next = b.gep(I8, stack, &[eight64], GepFlags::INBOUNDS)?;
    b.store(next, stack_p, 8)?;
    b.br(done)?;

    b.position_at_end(done);
    let addr = b.phi(PTR, &[(reg_addr, in_reg), (stack, on_stack)])?;
    let v = b.load(I32, addr, 8)?;
    b.store(v, pt, 4)?;
    b.call(va_end, &[ap])?;
    let t = b.load(I32, pt, 4)?;
    b.ret(Some(t))?;

    let hello = define(&mut m, "hello", VOID, &[], false);
    let printf = declare(&mut m, "printf", I32, &[PTR], true);
    let mut b = at_entry(&mut m, hello);
    let (one, two) = (int(&mut b, I32, 1), int(&mut b, I32, 2));
    let t = call_fn(&mut b, total, &[one, two])?;
    let x = b.module().const_float(F64, 1.5);
    call_fn(&mut b, printf, &[Value::global(fmt), t, x])?;
    b.ret_void()?;
    Ok(m)
}

/// §4/§6: variadic definitions and calls, `va_start`/`va_end` and the
/// aarch64 `va_arg` expansion with a `phi` of pointers.
#[test]
fn varargs() {
    differential("varargs", &build_varargs().unwrap());
}

fn build_floats() -> IrResult<Module> {
    let mut m = clang_module("floats");
    let f = define(&mut m, "mix", F64, &[F32, F64, I32, I32], false);
    let mut b = at_entry(&mut m, f);
    let [rhs, then, els, ret] = [(); 4].map(|()| b.append_block(""));
    let pret = b.alloca(F64, 8)?;
    let pf = b.alloca(F32, 4)?;
    let pd = b.alloca(F64, 8)?;
    let pi = b.alloca(I32, 4)?;
    let pu = b.alloca(I32, 4)?;
    let pg = b.alloca(F32, 4)?;
    let pr = b.alloca(F64, 8)?;
    for (n, (p, align)) in [(pf, 4), (pd, 8), (pi, 4), (pu, 4)].into_iter().enumerate() {
        b.store(arg(&b, n as u32), p, align)?;
    }
    let fv = b.load(F32, pf, 4)?;
    let two = b.module().const_float(F32, 2.0);
    let g = b.binop(BinOp::FMul, fv, two)?;
    b.store(g, pg, 4)?;
    let d = b.load(F64, pd, 8)?;
    let g = b.load(F32, pg, 4)?;
    let g = b.cast(CastOp::FPExt, g, F64)?;
    let q = b.binop(BinOp::FDiv, d, g)?;
    let i = b.load(I32, pi, 4)?;
    let i = b.cast(CastOp::SIToFP, i, F64)?;
    let s = b.binop(BinOp::FAdd, q, i)?;
    let u = b.load(I32, pu, 4)?;
    let u = b.cast(CastOp::UIToFP, u, F64)?;
    let r = b.binop(BinOp::FSub, s, u)?;
    b.store(r, pr, 8)?;
    let r = b.load(F64, pr, 8)?;
    let d = b.load(F64, pd, 8)?;
    let c = b.fcmp(FloatPredicate::Olt, r, d)?;
    b.cond_br(c, then, rhs)?;

    b.position_at_end(rhs);
    let r1 = b.load(F64, pr, 8)?;
    let r2 = b.load(F64, pr, 8)?;
    let c = b.fcmp(FloatPredicate::Une, r1, r2)?;
    b.cond_br(c, then, els)?;

    b.position_at_end(then);
    let r = b.load(F64, pr, 8)?;
    let l = b.cast(CastOp::FPToSI, r, I64)?;
    let l = b.cast(CastOp::SIToFP, l, F64)?;
    b.store(l, pret, 8)?;
    b.br(ret)?;

    b.position_at_end(els);
    let r = b.load(F64, pr, 8)?;
    let n = b.fneg(r, FastMath::NONE)?;
    b.store(n, pret, 8)?;
    b.br(ret)?;

    b.position_at_end(ret);
    let v = b.load(F64, pret, 8)?;
    b.ret(Some(v))?;
    Ok(m)
}

/// §3.2/§4: float constants, arithmetic, `fneg`, `fcmp` and FP casts.
#[test]
fn floats() {
    differential("floats", &build_floats().unwrap());
}

/// The `-O0` prologue: an alloca per parameter (`slots`) and per local
/// (`locals`), then a store of each parameter. Returns the allocas.
fn spill_args(
    b: &mut IrBuilder<'_>,
    slots: &[(TypeId, u64)],
    locals: &[(TypeId, u64)],
) -> IrResult<Vec<Value>> {
    let mut ps = Vec::with_capacity(slots.len() + locals.len());
    for &(ty, align) in slots.iter().chain(locals) {
        ps.push(b.alloca(ty, align)?);
    }
    for (n, (&p, &(_, align))) in ps.iter().zip(slots).enumerate() {
        b.store(arg(b, n as u32), p, align)?;
    }
    Ok(ps)
}

fn build_pointers() -> IrResult<Module> {
    let mut m = clang_module("pointers");
    let f = define(&mut m, "diff", I64, &[PTR, PTR], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(PTR, 8), (PTR, 8)], &[])?;
    let y = b.load(PTR, p[1], 8)?;
    let x = b.load(PTR, p[0], 8)?;
    let y = b.cast(CastOp::PtrToInt, y, I64)?;
    let x = b.cast(CastOp::PtrToInt, x, I64)?;
    let d = b.sub(y, x)?;
    let four = int(&mut b, I64, 4);
    let q = b.binop_ex(BinOp::SDiv, Flags::EXACT, FastMath::NONE, d, four)?;
    b.ret(Some(q))?;

    let f = define(&mut m, "step", PTR, &[PTR, I64], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(PTR, 8), (I64, 8)], &[])?;
    let base = b.load(PTR, p[0], 8)?;
    let i = b.load(I64, p[1], 8)?;
    let q = b.gep(I32, base, &[i], GepFlags::INBOUNDS)?;
    let m1 = int(&mut b, I64, -1);
    let q = b.gep(I32, q, &[m1], GepFlags::INBOUNDS)?;
    b.ret(Some(q))?;

    let f = define(&mut m, "deref", I32, &[PTR, I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(PTR, 8), (I32, 4)], &[])?;
    let pp = b.load(PTR, p[0], 8)?;
    let q = b.load(PTR, pp, 8)?;
    let i = b.load(I32, p[1], 4)?;
    let i = b.cast(CastOp::SExt, i, I64)?;
    let e = b.gep(I32, q, &[i], GepFlags::INBOUNDS)?;
    let v = b.load(I32, e, 4)?;
    b.ret(Some(v))?;

    let f = define(&mut m, "lt", I32, &[PTR, PTR], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(PTR, 8), (PTR, 8)], &[])?;
    let x = b.load(PTR, p[0], 8)?;
    let y = b.load(PTR, p[1], 8)?;
    let c = b.icmp(IntPredicate::Ult, x, y)?;
    let c = b.cast(CastOp::ZExt, c, I32)?;
    b.ret(Some(c))?;

    let f = define(&mut m, "addr", I64, &[PTR], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(PTR, 8)], &[])?;
    let x = b.load(PTR, p[0], 8)?;
    let x = b.cast(CastOp::PtrToInt, x, I64)?;
    b.ret(Some(x))?;
    Ok(m)
}

/// §4: pointer `icmp`, `ptrtoint`, `sdiv exact` and element GEPs with
/// negative indices.
#[test]
fn pointers() {
    differential("pointers", &build_pointers().unwrap());
}

fn build_bits() -> IrResult<Module> {
    let mut m = clang_module("bits");
    let f = define(&mut m, "lo", I8, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let x = b.load(I32, p[0], 4)?;
    let mask = int(&mut b, I32, 255);
    let x = b.binop(BinOp::And, x, mask)?;
    let x = b.cast(CastOp::Trunc, x, I8)?;
    b.ret(Some(x))?;

    let f = define(&mut m, "sh", I16, &[I16, I8], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I16, 2), (I8, 1)], &[])?;
    let a = b.load(I16, p[0], 2)?;
    let a = b.cast(CastOp::SExt, a, I32)?;
    let three = int(&mut b, I32, 3);
    let a = b.binop(BinOp::Shl, a, three)?;
    let c = b.load(I8, p[1], 1)?;
    let c = b.cast(CastOp::ZExt, c, I32)?;
    let one = int(&mut b, I32, 1);
    let c = b.binop(BinOp::AShr, c, one)?;
    let r = b.binop(BinOp::Or, a, c)?;
    let r = b.cast(CastOp::Trunc, r, I16)?;
    b.ret(Some(r))?;

    let f = define(&mut m, "rot", I32, &[I32, I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I32, 4), (I32, 4)], &[])?;
    let x = b.load(I32, p[0], 4)?;
    let n = b.load(I32, p[1], 4)?;
    let l = b.binop(BinOp::Shl, x, n)?;
    let x = b.load(I32, p[0], 4)?;
    let n = b.load(I32, p[1], 4)?;
    let w = int(&mut b, I32, 32);
    let k = b.sub(w, n)?;
    let r = b.binop(BinOp::LShr, x, k)?;
    let r = b.binop(BinOp::Or, l, r)?;
    b.ret(Some(r))?;

    let f = define(&mut m, "wide", I64, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let x = b.load(I32, p[0], 4)?;
    let x = b.cast(CastOp::ZExt, x, I64)?;
    let ones = int(&mut b, I64, -1);
    let nx = b.binop(BinOp::Xor, x, ones)?;
    let x = b.load(I32, p[0], 4)?;
    let seven = int(&mut b, I32, 7);
    let r = b.binop(BinOp::URem, x, seven)?;
    let r = b.cast(CastOp::ZExt, r, I64)?;
    let r = b.binop(BinOp::Xor, nx, r)?;
    b.ret(Some(r))?;

    let f = define(&mut m, "flag", I1, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let x = b.load(I32, p[0], 4)?;
    let zero = int(&mut b, I32, 0);
    let c = b.icmp(IntPredicate::Ne, x, zero)?;
    b.ret(Some(c))?;
    Ok(m)
}

/// §4: narrow integer types, shifts, bitwise operators and `urem`.
#[test]
fn bits() {
    differential("bits", &build_bits().unwrap());
}

/// `a && b` (`is_and`) or `a || b` as clang lowers it: a `phi i1`.
fn short_circuit(m: &mut Module, name: &str, is_and: bool) -> IrResult<()> {
    let f = define(m, name, I32, &[I32, I32], false);
    let mut b = at_entry(m, f);
    let entry = b.current_block().expect("positioned");
    let [rhs, end] = [(); 2].map(|()| b.append_block(""));
    let p = spill_args(&mut b, &[(I32, 4), (I32, 4)], &[])?;
    let zero = int(&mut b, I32, 0);
    let x = b.load(I32, p[0], 4)?;
    let c = b.icmp(IntPredicate::Ne, x, zero)?;
    if is_and {
        b.cond_br(c, rhs, end)?;
    } else {
        b.cond_br(c, end, rhs)?;
    }
    b.position_at_end(rhs);
    let y = b.load(I32, p[1], 4)?;
    let d = b.icmp(IntPredicate::Ne, y, zero)?;
    b.br(end)?;
    b.position_at_end(end);
    let short = b.module().const_bool(!is_and);
    let v = b.phi(I1, &[(short, entry), (d, rhs)])?;
    let v = b.cast(CastOp::ZExt, v, I32)?;
    b.ret(Some(v))?;
    Ok(())
}

fn build_logic() -> IrResult<Module> {
    let mut m = clang_module("logic");
    short_circuit(&mut m, "both", true)?;
    short_circuit(&mut m, "either", false)?;
    let f = define(&mut m, "pick", I32, &[I32, I32, I32], false);
    let mut b = at_entry(&mut m, f);
    let [t, e, end] = [(); 3].map(|()| b.append_block(""));
    let p = spill_args(&mut b, &[(I32, 4), (I32, 4), (I32, 4)], &[])?;
    let c = b.load(I32, p[0], 4)?;
    let zero = int(&mut b, I32, 0);
    let c = b.icmp(IntPredicate::Ne, c, zero)?;
    b.cond_br(c, t, e)?;
    b.position_at_end(t);
    let x = b.load(I32, p[1], 4)?;
    b.br(end)?;
    b.position_at_end(e);
    let y = b.load(I32, p[2], 4)?;
    b.br(end)?;
    b.position_at_end(end);
    let v = b.phi(I32, &[(x, t), (y, e)])?;
    b.ret(Some(v))?;
    Ok(m)
}

/// §4: `phi` with constant incoming values; the conditional operator.
#[test]
fn logic() {
    differential("logic", &build_logic().unwrap());
}

fn build_calls() -> IrResult<Module> {
    let mut m = clang_module("calls");
    let fib = define(&mut m, "fib", I32, &[I32], false);
    let sq = define(&mut m, "sq", I32, &[I32], false);
    let sqf = m.func_mut(sq);
    sqf.linkage = Linkage::Internal;
    sqf.dso_local = false;
    let apply = define(&mut m, "apply", I32, &[I32, I32], false);
    let table_ty = m.types.array(PTR, 2);
    let init = m.const_array(PTR, &[Value::func(sq), Value::func(fib)])?;
    let table = global(&mut m, "table", table_ty, init, 8);

    let mut b = at_entry(&mut m, fib);
    let [small, rec, end] = [(); 3].map(|()| b.append_block(""));
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let n = b.load(I32, p[0], 4)?;
    let two = int(&mut b, I32, 2);
    let c = b.icmp(IntPredicate::Slt, n, two)?;
    b.cond_br(c, small, rec)?;
    b.position_at_end(small);
    let n0 = b.load(I32, p[0], 4)?;
    b.br(end)?;
    b.position_at_end(rec);
    let mut parts = Vec::new();
    for k in [1, 2] {
        let n = b.load(I32, p[0], 4)?;
        let k = int(&mut b, I32, k);
        let a = b.sub_nsw(n, k)?;
        parts.push(call_fn(&mut b, fib, &[a])?);
    }
    let s = b.add_nsw(parts[0], parts[1])?;
    b.br(end)?;
    b.position_at_end(end);
    let v = b.phi(I32, &[(n0, small), (s, rec)])?;
    b.ret(Some(v))?;

    let mut b = at_entry(&mut m, sq);
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let x = b.load(I32, p[0], 4)?;
    let y = b.load(I32, p[0], 4)?;
    let r = b.mul_nsw(x, y)?;
    b.ret(Some(r))?;

    let fn_ty = m.types.function(I32, &[I32], false);
    let mut b = at_entry(&mut m, apply);
    let p = spill_args(&mut b, &[(I32, 4), (I32, 4)], &[])?;
    let i = b.load(I32, p[0], 4)?;
    let i = b.cast(CastOp::SExt, i, I64)?;
    let zero = int(&mut b, I64, 0);
    let slot = b.gep(table_ty, Value::global(table), &[zero, i], GepFlags::INBOUNDS)?;
    let target = b.load(PTR, slot, 8)?;
    let v = b.load(I32, p[1], 4)?;
    let x = call_noundef(&mut b, fn_ty, target, &[v])?;
    let v = b.load(I32, p[1], 4)?;
    let y = call_fn(&mut b, sq, &[v])?;
    let r = b.add_nsw(x, y)?;
    b.ret(Some(r))?;
    Ok(m)
}

/// §4: direct, recursive and indirect calls; an internal function; a
/// global array of function pointers.
#[test]
fn calls() {
    differential("calls", &build_calls().unwrap());
}

fn build_atomics() -> IrResult<Module> {
    let mut m = clang_module("atomics");
    let zero = m.const_int(I32, 0);
    let counter = Value::global(global(&mut m, "counter", I32, zero, 4));

    let f = define(&mut m, "bump", I32, &[], false);
    let mut b = at_entry(&mut m, f);
    let p1 = b.alloca(I32, 4)?;
    let p2 = b.alloca(I32, 4)?;
    let one = int(&mut b, I32, 1);
    b.store(one, p1, 4)?;
    let v = b.load(I32, p1, 4)?;
    let old = b.atomicrmw(RmwOp::Add, counter, v, ordered(4, Ordering::SeqCst))?;
    b.store(old, p2, 4)?;
    let r = b.load(I32, p2, 4)?;
    b.ret(Some(r))?;

    let f = define(&mut m, "get", I32, &[], false);
    let mut b = at_entry(&mut m, f);
    let p1 = b.alloca(I32, 4)?;
    let v = b.load_ex(I32, counter, ordered(4, Ordering::Acquire))?;
    b.store(v, p1, 4)?;
    let r = b.load(I32, p1, 4)?;
    b.ret(Some(r))?;

    let f = define(&mut m, "set", VOID, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I32, 4)], &[(I32, 4)])?;
    let v = b.load(I32, p[0], 4)?;
    b.store(v, p[1], 4)?;
    let v = b.load(I32, p[1], 4)?;
    b.store_ex(v, counter, ordered(4, Ordering::Release))?;
    b.ret_void()?;

    let f = define(&mut m, "barrier", VOID, &[], false);
    let mut b = at_entry(&mut m, f);
    b.fence(Ordering::AcqRel, SyncScope::SYSTEM)?;
    b.ret_void()?;

    let f = define(&mut m, "swap", I32, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I32, 4)], &[(I32, 4), (I32, 4)])?;
    let (tmp, res) = (p[1], p[2]);
    let v = b.load(I32, p[0], 4)?;
    b.store(v, tmp, 4)?;
    let v = b.load(I32, tmp, 4)?;
    let old = b.atomicrmw(RmwOp::Xchg, counter, v, ordered(4, Ordering::Monotonic))?;
    b.store(old, res, 4)?;
    let r = b.load(I32, res, 4)?;
    b.ret(Some(r))?;
    Ok(m)
}

/// §4: `atomicrmw`, atomic `load`/`store` and `fence`.
#[test]
fn atomics() {
    differential("atomics", &build_atomics().unwrap());
}

fn build_builtins() -> IrResult<Module> {
    let mut m = clang_module("builtins");
    let f = define(&mut m, "mulchk", I32, &[I64, I64, PTR], false);
    let umul = intrinsics::with_overflow(&mut m, OverflowOp::UMul, I64);
    let mut b = at_entry(&mut m, f);
    let p = spill_args(&mut b, &[(I64, 8), (I64, 8), (PTR, 8)], &[])?;
    let x = b.load(I64, p[0], 8)?;
    let y = b.load(I64, p[1], 8)?;
    let r = b.load(PTR, p[2], 8)?;
    let pair = b.call(umul, &[x, y])?;
    let ovf = b.extract_value(pair, &[1])?;
    let prod = b.extract_value(pair, &[0])?;
    b.store(prod, r, 8)?;
    let ovf = b.cast(CastOp::ZExt, ovf, I32)?;
    b.ret(Some(ovf))?;

    let f = define(&mut m, "die", VOID, &[I32], false);
    let trap = intrinsics::trap(&mut m);
    let mut b = at_entry(&mut m, f);
    let [then, end] = [(); 2].map(|()| b.append_block(""));
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let x = b.load(I32, p[0], 4)?;
    let x = b.cast(CastOp::SExt, x, I64)?;
    let zero = int(&mut b, I64, 0);
    let c = b.icmp(IntPredicate::Ne, x, zero)?;
    b.cond_br(c, then, end)?;
    b.position_at_end(then);
    b.call(trap, &[])?;
    b.br(end)?;
    b.position_at_end(end);
    b.ret_void()?;

    let f = define(&mut m, "never", I32, &[], false);
    let mut b = at_entry(&mut m, f);
    b.unreachable()?;
    Ok(m)
}

/// §6: `llvm.umul.with.overflow`, `extractvalue`, `llvm.trap`,
/// `unreachable`.
#[test]
fn builtins() {
    differential("builtins", &build_builtins().unwrap());
}

fn build_vla() -> IrResult<Module> {
    let mut m = clang_module("vla");
    let f = define(&mut m, "last", I32, &[I32], false);
    let save = intrinsics::stacksave(&mut m);
    let restore = intrinsics::stackrestore(&mut m);
    let mut b = at_entry(&mut m, f);
    let pn = b.alloca(I32, 4)?;
    let psave = b.alloca(PTR, 8)?;
    let plen = b.alloca(I64, 8)?;
    b.store(arg(&b, 0), pn, 4)?;
    let n = b.load(I32, pn, 4)?;
    let len = b.cast(CastOp::ZExt, n, I64)?;
    let sp = b.call(save, &[])?;
    b.store(sp, psave, 8)?;
    let arr = b.alloca_ex(I32, Some(len), 4, false)?;
    b.store(len, plen, 8)?;
    let v = b.load(I32, pn, 4)?;
    let one = int(&mut b, I32, 1);
    let elem = |b: &mut IrBuilder<'_>| -> IrResult<Value> {
        let n = b.load(I32, pn, 4)?;
        let i = b.sub_nsw(n, one)?;
        let i = b.cast(CastOp::SExt, i, I64)?;
        b.gep(I32, arr, &[i], GepFlags::INBOUNDS)
    };
    let e = elem(&mut b)?;
    b.store(v, e, 4)?;
    let e = elem(&mut b)?;
    let r = b.load(I32, e, 4)?;
    let sp = b.load(PTR, psave, 8)?;
    b.call(restore, &[sp])?;
    b.ret(Some(r))?;
    Ok(m)
}

/// §4/§6: a dynamically sized `alloca` between `llvm.stacksave` and
/// `llvm.stackrestore`.
#[test]
fn vla() {
    differential("vla", &build_vla().unwrap());
}

fn build_tls() -> IrResult<Module> {
    let mut m = clang_module("tls");
    let five = m.const_int(I32, 5);
    let tval = global(&mut m, "tval", I32, five, 4);
    m.global_mut(tval).thread_local = Some(TlsModel::GeneralDynamic);
    let zero = m.const_int(I64, 0);
    let tcount = global(&mut m, "tcount", I64, zero, 8);
    let gv = m.global_mut(tcount);
    gv.thread_local = Some(TlsModel::GeneralDynamic);
    gv.linkage = Linkage::Internal;
    gv.dso_local = false;

    let f = define(&mut m, "next", I32, &[], false);
    let tla = intrinsics::threadlocal_address(&mut m);
    let mut b = at_entry(&mut m, f);
    let address = |b: &mut IrBuilder<'_>, g: GlobalId, align: u64| -> IrResult<Value> {
        let a = b.module().attr_set(&[Attr::Align(align)]);
        let mut o = CallOpts::default();
        o.attrs.ret = a;
        o.attrs.set_param(0, a);
        let ty = b.module_ref().func(tla).ty();
        b.call_ex(ty, Value::func(tla), &[Value::global(g)], &o)
    };
    let pc = address(&mut b, tcount, 8)?;
    let c = b.load(I64, pc, 8)?;
    let one = int(&mut b, I64, 1);
    let c = b.add_nsw(c, one)?;
    b.store(c, pc, 8)?;
    let pv = address(&mut b, tval, 4)?;
    let v = b.load(I32, pv, 4)?;
    b.ret(Some(v))?;
    Ok(m)
}

/// §2.3/§6: `thread_local` globals accessed through
/// `llvm.threadlocal.address` with call-site return and parameter attrs.
#[test]
fn tls() {
    differential("tls", &build_tls().unwrap());
}

fn build_labels() -> IrResult<Module> {
    let mut m = clang_module("labels");
    let f = define(&mut m, "jump", I32, &[I32], false);
    let mut b = at_entry(&mut m, f);
    let entry = b.current_block().expect("positioned");
    let [la, lb, ret, dispatch] = [(); 4].map(|()| b.append_block(""));
    let targets_ty = b.module().types.array(PTR, 2);
    let ba = b.module().const_blockaddress(f, la);
    let bb = b.module().const_blockaddress(f, lb);
    let init = b.module().const_array(PTR, &[ba, bb])?;
    let targets = b.module().add_global("jump.targets", targets_ty);
    let gv = b.module().global_mut(targets);
    gv.linkage = Linkage::Internal;
    gv.align = Some(8);
    b.module().set_initializer(targets, Some(init))?;

    let pr = b.alloca(I32, 4)?;
    let p = spill_args(&mut b, &[(I32, 4)], &[])?;
    let i = b.load(I32, p[0], 4)?;
    let i = b.cast(CastOp::SExt, i, I64)?;
    let zero = int(&mut b, I64, 0);
    let slot = b.gep(targets_ty, Value::global(targets), &[zero, i], GepFlags::INBOUNDS)?;
    let addr = b.load(PTR, slot, 8)?;
    b.br(dispatch)?;
    for (bb, v) in [(la, 1), (lb, 2)] {
        b.position_at_end(bb);
        let v = int(&mut b, I32, v);
        b.store(v, pr, 4)?;
        b.br(ret)?;
    }
    b.position_at_end(ret);
    let r = b.load(I32, pr, 4)?;
    b.ret(Some(r))?;
    b.position_at_end(dispatch);
    let a = b.phi(PTR, &[(addr, entry)])?;
    b.indirect_br(a, &[la, lb])?;
    Ok(m)
}

/// §3.2/§4: `blockaddress` constants in a global and `indirectbr`.
#[test]
fn labels() {
    differential("labels", &build_labels().unwrap());
}
