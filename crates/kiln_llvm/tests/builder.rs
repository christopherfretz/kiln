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
