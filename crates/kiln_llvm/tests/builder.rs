//! Builder and verifier error paths, interning, determinism and layout
//! sizes (`spec/llvm.md` §3.1, §4.2, §7). Bad input is reported as an
//! [`IrError`], never a panic.

#![allow(clippy::many_single_char_names, clippy::similar_names, clippy::items_after_statements)]

mod common;

use kiln_llvm::*;

fn module() -> Module {
    Module::with_target("e.c", AARCH64_LINUX_GNU)
}

fn define<'m>(m: &'m mut Module, ret: TypeId, params: &[TypeId]) -> IrBuilder<'m> {
    let fty = m.types.function(ret, params, false);
    let f = m.add_function("f", fty).unwrap();
    let mut b = IrBuilder::new(m, f);
    let e = b.append_block("entry");
    b.position_at_end(e);
    b
}

fn kind<T: std::fmt::Debug>(r: IrResult<T>) -> IrErrorKind {
    r.expect_err("expected an error").kind
}

/// §3.1: handles stay small; `Inst` fits the 24-byte budget.
#[test]
fn layout_sizes() {
    use std::mem::size_of;
    assert_eq!(size_of::<Value>(), 4);
    assert_eq!(size_of::<Option<Value>>(), 8);
    assert_eq!(size_of::<TypeId>(), 4);
    assert_eq!(size_of::<BlockId>(), 4);
    assert_eq!(size_of::<MetadataId>(), 4);
    assert!(size_of::<Inst>() <= 24, "Inst is {} bytes", size_of::<Inst>());
}

/// §2.1: equal types share an id; named structs do not.
#[test]
fn type_interning() {
    let mut m = module();
    let a = m.types.array(TypeId::I32, 4);
    assert_eq!(a, m.types.array(TypeId::I32, 4));
    assert_ne!(a, m.types.array(TypeId::I32, 5));
    assert_eq!(m.types.int(32), TypeId::I32);
    assert_eq!(m.types.ptr(0), TypeId::PTR);
    let s1 = m.types.struct_literal(&[TypeId::I8, a], false);
    assert_eq!(s1, m.types.struct_literal(&[TypeId::I8, a], false));
    assert_ne!(s1, m.types.struct_literal(&[TypeId::I8, a], true));
    let n1 = m.types.named_struct("S");
    let n2 = m.types.named_struct("S");
    assert_ne!(n1, n2, "identified structs are distinct");
    assert!(m.types.set_body(n1, &[TypeId::VOID], false).is_err());
}

/// §3.2: constants are interned.
#[test]
fn constant_interning() {
    let mut m = module();
    assert_eq!(m.const_int(TypeId::I32, 7), m.const_int(TypeId::I32, 7));
    assert_ne!(m.const_int(TypeId::I32, 7), m.const_int(TypeId::I64, 7));
    assert_eq!(m.const_int(TypeId::I8, -1), m.const_int(TypeId::I8, 255));
    let s = m.const_cstring(b"x");
    assert_eq!(s, m.const_cstring(b"x"));
}

/// §4.2: operand type agreement.
#[test]
fn type_mismatches() {
    let mut m = module();
    let mut b = define(&mut m, TypeId::VOID, &[TypeId::I32, TypeId::I64, TypeId::DOUBLE]);
    let (x, y, d) = (b.func_ref().arg(0), b.func_ref().arg(1), b.func_ref().arg(2));
    assert_eq!(kind(b.add(x, y)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.binop(BinOp::FAdd, x, x)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.icmp(IntPredicate::Eq, d, d)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.fcmp(FloatPredicate::Oeq, x, x)), IrErrorKind::TypeMismatch);
    assert_eq!(
        kind(b.binop_ex(BinOp::Add, Flags::EXACT, FastMath::NONE, x, x)),
        IrErrorKind::InvalidFlags
    );
    assert_eq!(
        kind(b.binop_ex(BinOp::Add, Flags::NONE, FastMath::FAST, x, x)),
        IrErrorKind::InvalidFlags
    );
    assert_eq!(kind(b.store(x, y, 4)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.select(x, x, x)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.cast(CastOp::Trunc, x, TypeId::I64)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.cast(CastOp::ZExt, d, TypeId::I64)), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.ret(Some(x))), IrErrorKind::TypeMismatch);
    let fty = b.module().types.function(TypeId::VOID, &[TypeId::I32], false);
    let g = b.module().add_function("g", fty).unwrap();
    assert_eq!(kind(b.call(g, &[y])), IrErrorKind::TypeMismatch);
    assert_eq!(kind(b.call(g, &[])), IrErrorKind::BadCall);
    let s = b.module().types.struct_literal(&[TypeId::I32], false);
    assert_eq!(kind(b.gep(s, y, &[y], GepFlags::NONE)), IrErrorKind::TypeMismatch);
    let p = b.alloca(s, 4).unwrap();
    assert_eq!(kind(b.gep(s, p, &[y, y], GepFlags::NONE)), IrErrorKind::BadIndex);
    // Nothing invalid was inserted: the function is still well-formed.
    b.ret_void().unwrap();
    m.verify().unwrap();
    common::check(&m, &["  %3 = alloca { i32 }, align 4\n  ret void\n"]);
}

/// §4.2: atomics need valid orderings and alignment.
#[test]
fn bad_orderings() {
    let mut m = module();
    let mut b = define(&mut m, TypeId::VOID, &[TypeId::PTR, TypeId::I32]);
    let (p, x) = (b.func_ref().arg(0), b.func_ref().arg(1));
    let rel = MemOpts { align: 4, ordering: Ordering::Release, ..MemOpts::default() };
    assert_eq!(kind(b.load_ex(TypeId::I32, p, rel)), IrErrorKind::InvalidOrdering);
    let acq = MemOpts { ordering: Ordering::Acquire, ..rel };
    assert_eq!(kind(b.store_ex(x, p, acq)), IrErrorKind::InvalidOrdering);
    let noalign = MemOpts { align: 0, ..acq };
    assert_eq!(kind(b.load_ex(TypeId::I32, p, noalign)), IrErrorKind::InvalidOrdering);
    assert_eq!(kind(b.fence(Ordering::Monotonic, SyncScope::SYSTEM)), IrErrorKind::InvalidOrdering);
    let unord = MemOpts { ordering: Ordering::Unordered, ..rel };
    assert_eq!(kind(b.atomicrmw(RmwOp::Add, p, x, unord)), IrErrorKind::InvalidOrdering);
    let o = CmpXchgOpts {
        align: 4,
        success: Ordering::Monotonic,
        // (A failure ordering stronger than success is valid since LLVM 13.)
        failure: Ordering::Release,
        weak: false,
        volatile: false,
        scope: SyncScope::SYSTEM,
    };
    assert_eq!(kind(b.cmpxchg(p, x, x, o)), IrErrorKind::InvalidOrdering);
    assert_eq!(kind(b.alloca(TypeId::I32, 3)), IrErrorKind::InvalidOperand);
}

/// §4.2: one terminator per block, at the end.
#[test]
fn terminators() {
    let mut m = module();
    let mut b = define(&mut m, TypeId::VOID, &[]);
    b.ret_void().unwrap();
    assert!(b.is_terminated());
    assert_eq!(kind(b.ret_void()), IrErrorKind::TerminatorNotLast);
    assert_eq!(kind(b.alloca(TypeId::I32, 4)), IrErrorKind::TerminatorNotLast);
    let open = b.append_block("open");
    b.position_at_end(open);
    b.alloca(TypeId::I32, 4).unwrap();
    assert_eq!(kind(m.verify()), IrErrorKind::MissingTerminator);
    let msg = m.verify().unwrap_err().message;
    assert!(msg.contains("@f"), "{msg}");
}

/// §4.2: blocks must be placed and successors must exist.
#[test]
fn detached_blocks() {
    let mut m = module();
    let mut b = define(&mut m, TypeId::VOID, &[]);
    let floating = b.create_block("floating");
    b.br(floating).unwrap();
    assert_eq!(kind(m.verify()), IrErrorKind::DetachedBlock);
    let fid = m.get_function("f").unwrap();
    let mut b = IrBuilder::new(&mut m, fid);
    b.place_block(floating);
    b.position_at_end(floating);
    b.ret_void().unwrap();
    m.verify().unwrap();
}

/// §4.2: phi arity against predecessors, and phis first.
#[test]
fn phi_rules() {
    let mut m = module();
    let mut b = define(&mut m, TypeId::I32, &[TypeId::I1]);
    let c = b.func_ref().arg(0);
    let entry = b.current_block().unwrap();
    let t = b.append_block("t");
    let j = b.append_block("j");
    b.cond_br(c, t, j).unwrap();
    b.position_at_end(t);
    b.br(j).unwrap();
    b.position_at_end(j);
    let one = b.module().const_int(TypeId::I32, 1);
    let p = b.phi(TypeId::I32, &[(one, entry)]).unwrap();
    b.ret(Some(p)).unwrap();
    assert_eq!(kind(m.verify()), IrErrorKind::PhiArity);
    let fid = m.get_function("f").unwrap();
    let mut b = IrBuilder::new(&mut m, fid);
    b.add_incoming(p, one, t).unwrap();
    m.verify().unwrap();
    let mut b = IrBuilder::new(&mut m, fid);
    let bad = b.append_block("bad");
    b.position_at_end(bad);
    b.alloca(TypeId::I32, 4).unwrap();
    b.phi(TypeId::I32, &[]).unwrap();
    b.unreachable().unwrap();
    assert_eq!(kind(m.verify()), IrErrorKind::PhiNotFirst);
}

/// §4.2: the entry block has no predecessors; landingpads need a
/// personality.
#[test]
fn function_rules() {
    let mut m = module();
    let mut b = define(&mut m, TypeId::VOID, &[]);
    let entry = b.current_block().unwrap();
    b.br(entry).unwrap();
    assert_eq!(kind(m.verify()), IrErrorKind::BadFunction);

    let mut m = module();
    let lp = m.types.struct_literal(&[TypeId::PTR, TypeId::I32], false);
    let mut b = define(&mut m, TypeId::VOID, &[]);
    let pad = b.append_block("pad");
    b.unreachable().unwrap();
    b.position_at_end(pad);
    let v = b.landingpad(lp, true, &[]).unwrap();
    b.resume(v).unwrap();
    assert_eq!(kind(m.verify()), IrErrorKind::BadFunction);
}

/// §7: printing is deterministic: building the same module twice yields
/// identical bytes, and printing twice yields identical bytes.
#[test]
fn determinism() {
    fn build() -> String {
        let mut m = module();
        let s = m.types.named_struct("struct.S");
        m.types.set_body(s, &[TypeId::I32, TypeId::PTR], false).unwrap();
        let g = m.add_global("g", s);
        let z = m.const_zero(s);
        m.set_initializer(g, Some(z)).unwrap();
        let nu = m.attr_set(&[Attr::Flag(AttrKind::NoUnwind), Attr::string("k", "v")]);
        m.add_module_flag_i32(1, "wchar_size", 4);
        let mut b = define(&mut m, TypeId::I32, &[TypeId::I32]);
        b.func().attrs.func = nu;
        let x = b.func_ref().arg(0);
        let y = b.add(x, x).unwrap();
        b.ret(Some(y)).unwrap();
        m.to_text()
    }
    let a = build();
    assert_eq!(a, build());
    common::assert_fixed_point(&a);
}

/// `move_function_to_end` changes the print order, and with it the
/// numbering of attribute groups and unnamed functions, not the ids.
#[test]
fn move_function_to_end() {
    let mut m = module();
    let fty = m.types.function(TypeId::VOID, &[], false);
    let a = m.add_function("a", fty).unwrap();
    let b = m.add_function("b", fty).unwrap();
    let c = m.add_function("", fty).unwrap();
    let nounwind = m.attr_set(&[Attr::Flag(AttrKind::NoUnwind)]);
    let optnone = m.attr_set(&[Attr::Flag(AttrKind::OptimizeNone)]);
    m.func_mut(a).attrs.func = nounwind;
    m.func_mut(b).attrs.func = optnone;
    m.move_function_to_end(a);
    let order: Vec<FuncId> = m.functions().map(|(f, _)| f).collect();
    assert_eq!(order, [b, c, a]);
    let text = m.to_string();
    let (pb, pc, pa) = (
        text.find("declare void @b() #0").unwrap(),
        text.find("declare void @0()").unwrap(),
        text.find("declare void @a() #1").unwrap(),
    );
    assert!(pb < pc && pc < pa, "{text}");
    assert!(text.contains("attributes #0 = { optnone }"), "{text}");
    assert!(text.contains("attributes #1 = { nounwind }"), "{text}");
    assert_eq!(m.func(a).name(), "a");
}

fn call_and_ret(m: &mut Module, f: FuncId, callee: FuncId) {
    let mut b = IrBuilder::new(m, f);
    let e = b.append_block("entry");
    b.position_at_end(e);
    b.call(callee, &[]).unwrap();
    b.ret_void().unwrap();
}

/// `replace_function` (LLVM `replaceAllUsesWith` + `eraseFromParent`):
/// earlier and later uses print as the replacement, the replacement takes
/// the erased function's place in the order, and the name is freed.
#[test]
fn replace_function() {
    let mut m = module();
    let fty = m.types.function(TypeId::VOID, &[], false);
    let old = m.add_function("old", fty).unwrap();
    let user = m.add_function("user", fty).unwrap();
    let new = m.add_function("new", fty).unwrap();
    call_and_ret(&mut m, user, old);
    call_and_ret(&mut m, new, user);
    m.replace_function(old, Value::func(new)).unwrap();
    assert_eq!(m.resolve(Value::func(old)), Value::func(new));
    let order: Vec<FuncId> = m.functions().map(|(f, _)| f).collect();
    assert_eq!(order, [new, user]);
    let text = m.to_string();
    assert!(!text.contains("@old"), "{text}");
    assert!(text.contains("call void @new()"), "{text}");
    assert!(text.find("define void @new()").unwrap() < text.find("define void @user()").unwrap());
    assert_eq!(kind(m.replace_function(old, Value::func(new))), IrErrorKind::BadGlobal);
    assert_eq!(kind(m.replace_function(new, Value::func(user))), IrErrorKind::BadGlobal);
    assert!(m.add_function("old", fty).is_ok(), "the name is free again");
}

/// `replace_function_with_alias` (clang `TryEmitBaseDestructorAsAlias`):
/// the alias takes the declaration's name and its uses.
#[test]
fn replace_function_with_alias() {
    let mut m = module();
    let fty = m.types.function(TypeId::VOID, &[], false);
    let old = m.add_function("old", fty).unwrap();
    let user = m.add_function("user", fty).unwrap();
    let target = m.add_function("target", fty).unwrap();
    call_and_ret(&mut m, user, old);
    call_and_ret(&mut m, target, user);
    let a = m.replace_function_with_alias(old, fty, Value::func(target)).unwrap();
    assert_eq!(m.resolve(Value::func(old)), Value::alias(a));
    let text = m.to_string();
    assert!(text.contains("@old = alias void (), ptr @target"), "{text}");
    assert!(text.contains("call void @old()"), "{text}");
    assert!(!text.contains("declare void @old"), "{text}");
    assert_eq!(
        kind(m.replace_function_with_alias(target, fty, Value::func(target))),
        IrErrorKind::BadGlobal
    );
}

/// `replace_function_with_ifunc` / `add_ifunc` (clang
/// `emitIFuncDefinition`): the ifunc takes the declaration's name and
/// uses, and ifuncs print after every alias.
#[test]
fn replace_function_with_ifunc() {
    let mut m = module();
    let fty = m.types.function(TypeId::VOID, &[], false);
    let rty = m.types.function(TypeId::PTR, &[], false);
    let old = m.add_function("old", fty).unwrap();
    let user = m.add_function("user", fty).unwrap();
    let res = m.add_function("res", rty).unwrap();
    call_and_ret(&mut m, user, old);
    let i = m.replace_function_with_ifunc(old, fty, Value::func(res)).unwrap();
    assert!(m.alias(i).is_ifunc());
    m.add_alias("al", fty, Value::func(user)).unwrap();
    let text = m.to_string();
    assert!(text.contains("call void @old()"), "{text}");
    let (a, f) = (text.find("@al = alias").unwrap(), text.find("@old = ifunc void (), ptr @res"));
    assert!(f.is_some_and(|f| a < f), "{text}");
    assert_eq!(kind(m.add_ifunc("bad", TypeId::I32, Value::func(res))), IrErrorKind::InvalidType);
}

/// `replace_global_with_alias` (clang `EmitAliasDefinition` for a
/// variable): the alias takes the declaration's name and uses.
#[test]
fn replace_global_with_alias() {
    let mut m = module();
    let fty = m.types.function(TypeId::I32, &[], false);
    let g = m.add_global("g", TypeId::I32);
    let one = m.const_int(TypeId::I32, 1);
    m.set_initializer(g, Some(one)).unwrap();
    let old = m.add_global("va", TypeId::I32);
    let f = m.add_function("f", fty).unwrap();
    {
        let mut b = IrBuilder::new(&mut m, f);
        let e = b.append_block("entry");
        b.position_at_end(e);
        let v = b.load(TypeId::I32, Value::global(old), 4).unwrap();
        b.ret(Some(v)).unwrap();
    }
    let a = m.replace_global_with_alias(old, TypeId::I32, Value::global(g)).unwrap();
    assert_eq!(m.resolve(Value::global(old)), Value::alias(a));
    let text = m.to_string();
    assert!(text.contains("@va = alias i32, ptr @g"), "{text}");
    assert!(text.contains("load i32, ptr @va"), "{text}");
    assert!(!text.contains("@va = external"), "{text}");
    assert_eq!(
        kind(m.replace_global_with_alias(g, TypeId::I32, Value::alias(a))),
        IrErrorKind::BadGlobal
    );
}

/// LLVM 20's `FPMathOperator`: fast-math flags on `fpext`/`fptrunc` and
/// on calls returning a homogeneous FP struct, only at
/// [`Module::ir_version`] 20 or later (WP-806).
#[test]
fn fp_math_operators_by_version() {
    /// The module text, and whether the homogeneous-struct call and the
    /// mixed-struct call took the flags.
    fn build(ir_version: u32) -> (String, bool, bool) {
        let mut m = module();
        m.ir_version = ir_version;
        let cd = m.types.struct_literal(&[TypeId::DOUBLE, TypeId::DOUBLE], false);
        let mixed = m.types.struct_literal(&[TypeId::FLOAT, TypeId::DOUBLE], false);
        let mty = m.types.function(cd, &[TypeId::DOUBLE], false);
        let mul = m.add_function("mul", mty).unwrap();
        let xty = m.types.function(mixed, &[], false);
        let mx = m.add_function("mixed", xty).unwrap();
        let mut b = define(&mut m, TypeId::FLOAT, &[TypeId::FLOAT]);
        let x = b.func_ref().arg(0);
        let d = b.cast_fmf(CastOp::FPExt, FastMath::CONTRACT, x, TypeId::DOUBLE).unwrap();
        let o = CallOpts { fmf: FastMath::NNAN, ..CallOpts::default() };
        let mixed_ok = b.call_ex(xty, Value::func(mx), &[], &o).is_ok();
        let homog_ok = b.call_ex(mty, Value::func(mul), &[d], &o).is_ok();
        let f = b.cast_fmf(CastOp::FPTrunc, FastMath::FAST, d, TypeId::FLOAT).unwrap();
        b.ret(Some(f)).unwrap();
        (m.to_text(), homog_ok, mixed_ok)
    }
    let (t, homog, mixed) = build(19);
    assert!(t.contains("fpext float %0 to double") && t.contains("fptrunc double"), "{t}");
    assert!(!homog && !mixed);
    let (t, homog, mixed) = build(20);
    assert!(t.contains("fpext contract float %0 to double"), "{t}");
    assert!(t.contains("fptrunc fast double") && t.contains("call nnan { double, double }"), "{t}");
    assert!(homog && !mixed);
}
