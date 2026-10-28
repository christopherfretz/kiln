//! Hand-built modules, one per feature group, each printed and round-tripped
//! through `llvm-as-19` and `opt-19 -passes=verify` (`spec/llvm.md` §9.1).
//! Every test also pins the exact text of the lines it exercises.

mod common;

use common::check;
use kiln_llvm::*;

fn aarch64() -> Module {
    Module::with_target("t.c", AARCH64_LINUX_GNU)
}

/// §2.2: module header and an empty module.
#[test]
fn header() {
    let m = aarch64();
    let t = check(
        &m,
        &[
            "; ModuleID = 't.c'\nsource_filename = \"t.c\"\n",
            "target datalayout = \"e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n",
            "target triple = \"aarch64-unknown-linux-gnu\"\n",
        ],
    );
    assert!(t.ends_with("target triple = \"aarch64-unknown-linux-gnu\"\n"));
}

/// §4.1: a definition with arguments, arithmetic and `ret`.
#[test]
fn simple_function() {
    let mut m = aarch64();
    let fty = m.types.function(TypeId::I32, &[TypeId::I32, TypeId::I32], false);
    let f = m.add_function("add", fty).unwrap();
    m.func_mut(f).dso_local = true;
    let mut b = IrBuilder::new(&mut m, f);
    let entry = b.append_block("");
    b.position_at_end(entry);
    let (x, y) = (b.func_ref().arg(0), b.func_ref().arg(1));
    let s = b.add_nsw(x, y).unwrap();
    let t = b.mul(s, x).unwrap();
    b.ret(Some(t)).unwrap();
    check(
        &m,
        &[
            "\ndefine dso_local i32 @add(i32 %0, i32 %1) {\n",
            "  %3 = add nsw i32 %0, %1\n  %4 = mul i32 %3, %0\n  ret i32 %4\n}\n",
        ],
    );
}

/// Build a `define` of type `ret(params)` named `name` and return a builder
/// positioned in its entry block.
fn define<'m>(m: &'m mut Module, name: &str, ret: TypeId, params: &[TypeId]) -> IrBuilder<'m> {
    let fty = m.types.function(ret, params, false);
    let f = m.add_function(name, fty).unwrap();
    m.func_mut(f).dso_local = true;
    let mut b = IrBuilder::new(m, f);
    let e = b.append_block("");
    b.position_at_end(e);
    b
}

fn arg(b: &IrBuilder<'_>, n: u32) -> Value {
    b.func_ref().arg(n)
}

/// §2.2: the x86-64 target strings.
#[test]
fn x86_64_header() {
    let m = Module::with_target("x.c", X86_64_LINUX_GNU);
    check(&m, &["target triple = \"x86_64-unknown-linux-gnu\""]);
}

/// §4.1: names, uniquing, quoting, and numbering around named values.
#[test]
fn names_and_numbering() {
    let mut m = aarch64();
    let mut b = define(&mut m, "names", TypeId::I32, &[TypeId::I32, TypeId::I32]);
    b.func().set_arg_name(0, "x");
    let (x, y) = (arg(&b, 0), arg(&b, 1));
    let e = b.current_block().unwrap();
    b.func().set_block_name(e, "entry");
    b.name_next("sum");
    let s1 = b.add(x, y).unwrap();
    b.name_next("sum");
    let s2 = b.add(s1, y).unwrap();
    let s3 = b.add(s2, x).unwrap();
    b.name_next("a b");
    let s4 = b.add(s3, x).unwrap();
    b.name_next("1x");
    let s5 = b.add(s4, x).unwrap();
    let next = b.append_block("");
    b.br(next).unwrap();
    b.position_at_end(next);
    let s6 = b.sub(s5, s3).unwrap();
    b.ret(Some(s6)).unwrap();
    check(
        &m,
        &[
            "define dso_local i32 @names(i32 %x, i32 %0) {\nentry:\n",
            "  %sum = add i32 %x, %0\n",
            "  %sum1 = add i32 %sum, %0\n",
            "  %1 = add i32 %sum1, %x\n",
            "  %\"a b\" = add i32 %1, %x\n",
            "  %\"1x\" = add i32 %\"a b\", %x\n",
            "  br label %2\n\n2:\n",
            "  %3 = sub i32 %\"1x\", %1\n",
        ],
    );
}

/// §2.1: identified (named, unnamed, opaque, packed) and literal structs,
/// arrays and vectors; definitions print in `TypeFinder` order.
#[test]
fn struct_types() {
    let mut m = aarch64();
    let t = &mut m.types;
    let s = t.named_struct("struct.S");
    let inner = t.named_struct("struct.Inner");
    let opaque = t.named_struct("struct.Opaque");
    let anon = t.named_struct("");
    let unused = t.named_struct("struct.Unused");
    let arr = t.array(TypeId::I8, 4);
    t.set_body(inner, &[TypeId::I16, arr], false).unwrap();
    let v4 = t.vector(TypeId::FLOAT, 4);
    let lit = t.struct_literal(&[TypeId::I32, TypeId::DOUBLE], false);
    t.set_body(s, &[TypeId::I32, inner, TypeId::PTR, v4, lit], false).unwrap();
    t.set_body(anon, &[TypeId::I8, TypeId::I64], true).unwrap();
    t.set_body(unused, &[TypeId::I8], false).unwrap();
    let a2 = t.array(s, 2);
    m.add_global("g", a2);
    m.add_global("o", opaque);
    m.add_global("p", anon);
    for n in ["g", "o", "p"] {
        let g = m.get_global(n).unwrap();
        m.global_mut(g).dso_local = true;
    }
    let text = check(
        &m,
        &[
            "\n%0 = type <{ i8, i64 }>\n",
            "%struct.S = type { i32, %struct.Inner, ptr, <4 x float>, { i32, double } }\n",
            "%struct.Inner = type { i16, [4 x i8] }\n",
            "%struct.Opaque = type opaque\n",
            "\n@g = external dso_local global [2 x %struct.S]\n",
            "@o = external dso_local global %struct.Opaque\n",
            "@p = external dso_local global %0\n",
        ],
    );
    assert!(!text.contains("struct.Unused"));
}

/// §3.2: integer constants of every width, `true`/`false`, wide values.
#[test]
fn int_constants() {
    let mut m = aarch64();
    let i256 = m.types.int(256);
    let i7 = m.types.int(7);
    let vals = [
        (TypeId::I1, m.const_int(TypeId::I1, 1)),
        (TypeId::I1, m.const_bool(false)),
        (TypeId::I8, m.const_int(TypeId::I8, 255)),
        (i7, m.const_int(i7, -1)),
        (TypeId::I64, m.const_int(TypeId::I64, i128::from(i64::MIN))),
        (TypeId::I128, m.const_int(TypeId::I128, i128::MAX)),
        (i256, m.const_int_words(i256, &[0, 0, 0, 1 << 63])),
        (i256, m.const_int_words(i256, &[5, 0, 0, 0])),
    ];
    for (i, (ty, v)) in vals.into_iter().enumerate() {
        let g = m.add_global(&format!("c{i}"), ty);
        m.set_initializer(g, Some(v)).unwrap();
    }
    check(
        &m,
        &[
            "@c0 = global i1 true\n",
            "@c1 = global i1 false\n",
            "@c2 = global i8 -1\n",
            "@c3 = global i7 -1\n",
            "@c4 = global i64 -9223372036854775808\n",
            "@c5 = global i128 170141183460469231731687303715884105727\n",
            "@c6 = global i256 -57896044618658097711785492504343953926634992332820282019728792003956564819968\n",
            "@c7 = global i256 5\n",
        ],
    );
}

/// §3.2: floating-point constants in every format, decimal and hex forms.
#[test]
fn float_constants() {
    let mut m = Module::with_target("f.c", X86_64_LINUX_GNU);
    let vals = [
        (TypeId::DOUBLE, m.const_float(TypeId::DOUBLE, 1.0)),
        (TypeId::DOUBLE, m.const_float(TypeId::DOUBLE, 0.1)),
        (TypeId::DOUBLE, m.const_float(TypeId::DOUBLE, 1.0 / 3.0)),
        (TypeId::DOUBLE, m.const_float(TypeId::DOUBLE, -2.5e-300)),
        (TypeId::DOUBLE, m.const_float(TypeId::DOUBLE, f64::INFINITY)),
        (TypeId::FLOAT, m.const_float(TypeId::FLOAT, 0.5)),
        (TypeId::FLOAT, m.const_float(TypeId::FLOAT, 0.1)),
        (TypeId::FLOAT, m.const_float(TypeId::FLOAT, f64::NAN)),
        (TypeId::HALF, m.const_float(TypeId::HALF, 1.0)),
        (TypeId::BFLOAT, m.const_float(TypeId::BFLOAT, 1.0)),
        (TypeId::FP128, m.const_float(TypeId::FP128, 1.0)),
        (TypeId::X86_FP80, m.const_float(TypeId::X86_FP80, 1.0)),
        (TypeId::DOUBLE, m.const_float(TypeId::DOUBLE, -0.0)),
    ];
    for (i, (ty, v)) in vals.into_iter().enumerate() {
        let g = m.add_global(&format!("f{i}"), ty);
        m.set_initializer(g, Some(v)).unwrap();
    }
    check(
        &m,
        &[
            "@f0 = global double 1.000000e+00\n",
            "@f1 = global double 1.000000e-01\n",
            "@f2 = global double 0x3FD5555555555555\n",
            "@f3 = global double -2.500000e-300\n",
            "@f4 = global double 0x7FF0000000000000\n",
            "@f5 = global float 5.000000e-01\n",
            "@f6 = global float 0x3FB99999A0000000\n",
            "@f7 = global float 0x7FF8000000000000\n",
            "@f8 = global half 0xH3C00\n",
            "@f9 = global bfloat 0xR3F80\n",
            "@f10 = global fp128 0xL00000000000000003FFF000000000000\n",
            "@f11 = global x86_fp80 0xK3FFF8000000000000000\n",
            "@f12 = global double -0.000000e+00\n",
        ],
    );
}

/// §2.3: linkage, visibility, `dso_local`, TLS, `unnamed_addr`, section,
/// alignment, address space, `externally_initialized`.
#[test]
fn global_properties() {
    let mut m = aarch64();
    let one = m.const_int(TypeId::I32, 1);
    let mk = |m: &mut Module, name: &str, f: &dyn Fn(&mut GlobalVar)| {
        let g = m.add_global(name, TypeId::I32);
        m.set_initializer(g, Some(one)).unwrap();
        f(m.global_mut(g));
        g
    };
    mk(&mut m, "internal", &|g| g.linkage = Linkage::Internal);
    mk(&mut m, "private", &|g| {
        g.linkage = Linkage::Private;
        g.unnamed_addr = UnnamedAddr::Global;
        g.is_constant = true;
        g.align = Some(4);
    });
    mk(&mut m, "weak", &|g| {
        g.linkage = Linkage::Weak;
        g.dso_local = true;
    });
    mk(&mut m, "lodr", &|g| {
        g.linkage = Linkage::LinkOnceOdr;
        g.visibility = Visibility::Hidden;
        g.dso_local = true;
    });
    mk(&mut m, "wodr", &|g| {
        g.linkage = Linkage::WeakOdr;
        g.visibility = Visibility::Protected;
        g.dso_local = true;
    });
    mk(&mut m, "tls", &|g| {
        g.thread_local = Some(TlsModel::GeneralDynamic);
        g.dso_local = true;
    });
    mk(&mut m, "tls_ie", &|g| g.thread_local = Some(TlsModel::InitialExec));
    mk(&mut m, "tls_le", &|g| g.thread_local = Some(TlsModel::LocalExec));
    mk(&mut m, "tls_ld", &|g| {
        g.thread_local = Some(TlsModel::LocalDynamic);
        g.linkage = Linkage::Internal;
    });
    mk(&mut m, "sect", &|g| {
        g.section = Some("my \"sec\"".into());
        g.unnamed_addr = UnnamedAddr::Local;
        g.align = Some(16);
    });
    mk(&mut m, "ext_init", &|g| g.externally_initialized = true);
    let z = m.const_zero(TypeId::I32);
    let c = m.add_global("common", TypeId::I32);
    m.set_initializer(c, Some(z)).unwrap();
    m.global_mut(c).linkage = Linkage::Common;
    m.global_mut(c).align = Some(4);
    let ew = m.add_global("ew", TypeId::I32);
    m.global_mut(ew).linkage = Linkage::ExternWeak;
    let a1 = m.add_global("as1", TypeId::I32);
    m.set_global_addrspace(a1, 1);
    check(
        &m,
        &[
            "@internal = internal global i32 1\n",
            "@private = private unnamed_addr constant i32 1, align 4\n",
            "@weak = weak dso_local global i32 1\n",
            "@lodr = linkonce_odr hidden global i32 1\n",
            "@wodr = weak_odr protected global i32 1\n",
            "@tls = dso_local thread_local global i32 1\n",
            "@tls_ie = thread_local(initialexec) global i32 1\n",
            "@tls_le = thread_local(localexec) global i32 1\n",
            "@tls_ld = internal thread_local(localdynamic) global i32 1\n",
            "@sect = local_unnamed_addr global i32 1, section \"my \\22sec\\22\", align 16\n",
            "@ext_init = externally_initialized global i32 1\n",
            "@common = common global i32 0, align 4\n",
            "@ew = extern_weak global i32\n",
            "@as1 = external addrspace(1) global i32\n",
        ],
    );
}

/// §3.2: aggregate initializers, strings, `zeroinitializer`, `undef`,
/// `poison`.
#[test]
fn aggregate_initializers() {
    let mut m = aarch64();
    let s = m.types.named_struct("struct.P");
    m.types.set_body(s, &[TypeId::I8, TypeId::I32], false).unwrap();
    let ints: Vec<Value> = (1..=3).map(|i| m.const_int(TypeId::I32, i)).collect();
    let arr = m.const_array(TypeId::I32, &ints).unwrap();
    let c8 = m.const_int(TypeId::I8, 7);
    let st = m.const_struct(s, &[c8, ints[0]]).unwrap();
    let packed = m.const_anon_struct(&[c8, ints[1]], true).unwrap();
    let vec = m.const_vector(&ints).unwrap();
    let text = m.const_cstring(b"hi\n\"q\"\\");
    let raw = m.const_bytes(&[0, 1, 255]);
    let arr_ty = m.types.array(TypeId::I32, 3);
    let zero = m.const_zero(arr_ty);
    let undef = m.const_undef(s);
    let poison = m.const_poison(TypeId::I64);
    let empty = m.const_anon_struct(&[], false).unwrap();
    let null = m.const_null(TypeId::PTR);
    let sarr_ty = m.types.array(s, 2);
    let sarr = m.const_array(s, &[st, undef]).unwrap();
    let inits = [arr, st, packed, vec, text, raw, zero, undef, poison, empty, null, sarr];
    for (i, v) in inits.into_iter().enumerate() {
        let ty = m.const_type(v);
        let g = m.add_global(&format!("a{i}"), ty);
        m.set_initializer(g, Some(v)).unwrap();
    }
    let _ = sarr_ty;
    check(
        &m,
        &[
            "@a0 = global [3 x i32] [i32 1, i32 2, i32 3]\n",
            "@a1 = global %struct.P { i8 7, i32 1 }\n",
            "@a2 = global <{ i8, i32 }> <{ i8 7, i32 2 }>\n",
            "@a3 = global <3 x i32> <i32 1, i32 2, i32 3>\n",
            "@a4 = global [8 x i8] c\"hi\\0A\\22q\\22\\\\\\00\"\n",
            "@a5 = global [3 x i8] c\"\\00\\01\\FF\"\n",
            "@a6 = global [3 x i32] zeroinitializer\n",
            "@a7 = global %struct.P undef\n",
            "@a8 = global i64 poison\n",
            "@a9 = global {} zeroinitializer\n",
            "@a10 = global ptr null\n",
            "@a11 = global [2 x %struct.P] [%struct.P { i8 7, i32 1 }, %struct.P undef]\n",
        ],
    );
}

/// §3.2: constant expressions (opaque-pointer forms).
#[test]
fn constant_expressions() {
    let mut m = aarch64();
    let arr = m.types.array(TypeId::I32, 4);
    let g = m.add_global("arr", arr);
    let z = m.const_zero(arr);
    m.set_initializer(g, Some(z)).unwrap();
    let gv = Value::global(g);
    let (c0, c2) = (m.const_int(TypeId::I64, 0), m.const_int(TypeId::I32, 2));
    let e = m.const_gep(arr, gv, &[c0, c2], GepFlags::INBOUNDS, None).unwrap();
    let p = m.add_global("p", TypeId::PTR);
    m.set_initializer(p, Some(e)).unwrap();
    let c1 = m.const_int(TypeId::I64, 1);
    let r = m.const_gep(arr, gv, &[c0, c1], GepFlags::INBOUNDS, Some((-4, 12))).unwrap();
    let q = m.add_global("q", TypeId::PTR);
    m.set_initializer(q, Some(r)).unwrap();
    let pi = m.const_cast(CastOp::PtrToInt, gv, TypeId::I64).unwrap();
    let i = m.add_global("i", TypeId::I64);
    m.set_initializer(i, Some(pi)).unwrap();
    let k = m.const_int(TypeId::I64, 8);
    let s = m.const_binop(BinOp::Add, Flags::NUW, pi, k).unwrap();
    let j = m.add_global("j", TypeId::I64);
    m.set_initializer(j, Some(s)).unwrap();
    let ip = m.const_int(TypeId::I64, 4096);
    let ip = m.const_cast(CastOp::IntToPtr, ip, TypeId::PTR).unwrap();
    let h = m.add_global("h", TypeId::PTR);
    m.set_initializer(h, Some(ip)).unwrap();
    let p1 = m.types.ptr(1);
    let asc = m.const_cast(CastOp::AddrSpaceCast, gv, p1).unwrap();
    let a = m.add_global("as", p1);
    m.set_initializer(a, Some(asc)).unwrap();
    let t = m.const_cast(CastOp::Trunc, pi, TypeId::I32).unwrap();
    let tt = m.add_global("t", TypeId::I32);
    m.set_initializer(tt, Some(t)).unwrap();
    check(
        &m,
        &[
            "@arr = global [4 x i32] zeroinitializer\n",
            "@p = global ptr getelementptr inbounds ([4 x i32], ptr @arr, i64 0, i32 2)\n",
            "@q = global ptr getelementptr inbounds inrange(-4, 12) ([4 x i32], ptr @arr, i64 0, i64 1)\n",
            "@i = global i64 ptrtoint (ptr @arr to i64)\n",
            "@j = global i64 add nuw (i64 ptrtoint (ptr @arr to i64), i64 8)\n",
            "@h = global ptr inttoptr (i64 4096 to ptr)\n",
            "@as = global ptr addrspace(1) addrspacecast (ptr @arr to ptr addrspace(1))\n",
            "@t = global i32 trunc (i64 ptrtoint (ptr @arr to i64) to i32)\n",
        ],
    );
}

/// §2.5: aliases (plain, GEP aliasee, properties) and comdats.
#[test]
fn aliases_and_comdats() {
    let mut m = aarch64();
    let s = m.types.struct_literal(&[TypeId::I32, TypeId::I32], false);
    let g = m.add_global("g", s);
    let z = m.const_zero(s);
    m.set_initializer(g, Some(z)).unwrap();
    let c = m.comdat("g", ComdatKind::Any);
    m.global_mut(g).comdat = Some(c);
    m.global_mut(g).linkage = Linkage::LinkOnceOdr;
    let c2 = m.comdat("grp", ComdatKind::Largest);
    let h = m.add_global("h", TypeId::I8);
    let z8 = m.const_int(TypeId::I8, 0);
    m.set_initializer(h, Some(z8)).unwrap();
    m.global_mut(h).comdat = Some(c2);
    let _ = m.comdat("nd", ComdatKind::NoDeduplicate);
    let gv = Value::global(g);
    let a = m.add_alias("a", s, gv).unwrap();
    m.alias_mut(a).dso_local = true;
    let (c0, c1) = (m.const_int(TypeId::I32, 0), m.const_int(TypeId::I32, 1));
    let fld = m.const_gep(s, gv, &[c0, c1], GepFlags::INBOUNDS, None).unwrap();
    let b = m.add_alias("b", TypeId::I32, fld).unwrap();
    {
        let b = m.alias_mut(b);
        b.linkage = Linkage::Internal;
        b.unnamed_addr = UnnamedAddr::Global;
    }
    let w = m.add_alias("w", s, Value::alias(a)).unwrap();
    m.alias_mut(w).linkage = Linkage::Weak;
    m.alias_mut(w).visibility = Visibility::Hidden;
    let fty = m.types.function(TypeId::VOID, &[], false);
    let f = m.add_function("f", fty).unwrap();
    m.func_mut(f).comdat = Some(c);
    m.func_mut(f).linkage = Linkage::LinkOnceOdr;
    let mut bld = IrBuilder::new(&mut m, f);
    let e = bld.append_block("");
    bld.position_at_end(e);
    bld.ret_void().unwrap();
    check(
        &m,
        &[
            // Comdats are separated by blank lines; unused ones are not printed.
            "\n$g = comdat any\n\n$grp = comdat largest\n\n@g",
            " = linkonce_odr global { i32, i32 } zeroinitializer, comdat\n",
            "@h = global i8 0, comdat($grp)\n",
            "@a = dso_local alias { i32, i32 }, ptr @g\n",
            "@b = internal unnamed_addr alias i32, getelementptr inbounds ({ i32, i32 }, ptr @g, i32 0, i32 1)\n",
            "@w = weak hidden alias { i32, i32 }, ptr @a\n",
            "define linkonce_odr void @f() comdat($g) {\n",
        ],
    );
}

/// §3.2: `blockaddress`, `dso_local_equivalent`, `no_cfi`.
#[test]
fn function_constants() {
    let mut m = aarch64();
    let fty = m.types.function(TypeId::VOID, &[], false);
    let ext = m.add_function("ext", fty).unwrap();
    let mut b = define(&mut m, "f", TypeId::VOID, &[]);
    let fid = b.func_id();
    let l = b.append_block("lbl");
    b.br(l).unwrap();
    b.position_at_end(l);
    b.ret_void().unwrap();
    let ba = m.const_blockaddress(fid, l);
    let g = m.add_global("ba", TypeId::PTR);
    m.set_initializer(g, Some(ba)).unwrap();
    let d = m.const_dso_local_equivalent(Value::func(ext));
    let g = m.add_global("dle", TypeId::PTR);
    m.set_initializer(g, Some(d)).unwrap();
    let n = m.const_no_cfi(Value::func(ext));
    let g = m.add_global("ncfi", TypeId::PTR);
    m.set_initializer(g, Some(n)).unwrap();
    check(
        &m,
        &[
            "@ba = global ptr blockaddress(@f, %lbl)\n",
            "@dle = global ptr dso_local_equivalent @ext\n",
            "@ncfi = global ptr no_cfi @ext\n",
            "declare void @ext()\n",
            "  br label %lbl\n\nlbl:",
        ],
    );
}

/// §2.4: declarations, varargs, unnamed functions and globals.
#[test]
fn declarations() {
    let mut m = aarch64();
    let pf = m.types.function(TypeId::I32, &[TypeId::PTR], true);
    let printf = m.add_function("printf", pf).unwrap();
    let nounwind = m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
    m.func_mut(printf).attrs.set_param(0, nounwind);
    let vf = m.types.function(TypeId::VOID, &[], true);
    let _ = m.add_function("only_va", vf).unwrap();
    let ew = m.add_function("weak_ref", vf).unwrap();
    m.func_mut(ew).linkage = Linkage::ExternWeak;
    let anon = m.add_function("", vf).unwrap();
    m.func_mut(anon).linkage = Linkage::Internal;
    let mut b = IrBuilder::new(&mut m, anon);
    let e = b.append_block("");
    b.position_at_end(e);
    b.ret_void().unwrap();
    let g = m.add_global("", TypeId::I32);
    let one = m.const_int(TypeId::I32, 1);
    m.set_initializer(g, Some(one)).unwrap();
    m.global_mut(g).linkage = Linkage::Private;
    let user = m.add_global("user", TypeId::PTR);
    m.set_initializer(user, Some(Value::func(anon))).unwrap();
    let user2 = m.add_global("user2", TypeId::PTR);
    m.set_initializer(user2, Some(Value::global(g))).unwrap();
    check(
        &m,
        &[
            "@0 = private global i32 1\n@user = global ptr @1\n@user2 = global ptr @0\n",
            "declare i32 @printf(ptr noundef, ...)\n",
            "declare void @only_va(...)\n",
            "declare extern_weak void @weak_ref(...)\n",
            "define internal void @1(...) {\n",
        ],
    );
}

/// §5: parameter and return attributes as clang emits them.
#[test]
fn parameter_attributes() {
    let mut m = aarch64();
    let s = m.types.named_struct("struct.S");
    m.types.set_body(s, &[TypeId::I64, TypeId::I64, TypeId::I64], false).unwrap();
    let params = [TypeId::PTR, TypeId::I8, TypeId::I16, TypeId::PTR, TypeId::PTR, TypeId::PTR];
    let fty = m.types.function(TypeId::VOID, &params, false);
    let f = m.add_function("f", fty).unwrap();
    let sets = [
        m.attr_set(&[
            Attr::Flag(AttrKind::NoAlias),
            Attr::Flag(AttrKind::Writable),
            Attr::SRet(s),
            Attr::Align(8),
        ]),
        m.attr_set(&[Attr::Flag(AttrKind::NoUndef), Attr::Flag(AttrKind::ZExt)]),
        m.attr_set(&[Attr::Flag(AttrKind::NoUndef), Attr::Flag(AttrKind::SExt)]),
        m.attr_set(&[Attr::ByVal(s), Attr::Align(8)]),
        m.attr_set(&[
            Attr::Flag(AttrKind::NoUndef),
            Attr::Flag(AttrKind::NonNull),
            Attr::Align(4),
            Attr::Dereferenceable(16),
        ]),
        m.attr_set(&[
            Attr::Flag(AttrKind::NoCapture),
            Attr::Flag(AttrKind::ReadOnly),
            Attr::DereferenceableOrNull(8),
        ]),
    ];
    for (i, a) in sets.into_iter().enumerate() {
        m.func_mut(f).attrs.set_param(i, a);
    }
    let gty = m.types.function(TypeId::PTR, &[TypeId::I1], false);
    let g = m.add_function("g", gty).unwrap();
    let r = m.attr_set(&[Attr::Flag(AttrKind::NoAlias), Attr::Flag(AttrKind::NonNull)]);
    m.func_mut(g).attrs.ret = r;
    let z = m.attr_set(&[Attr::Flag(AttrKind::ZExt)]);
    m.func_mut(g).attrs.set_param(0, z);
    check(
        &m,
        &[
            "%struct.S = type { i64, i64, i64 }\n",
            "declare void @f(ptr noalias writable sret(%struct.S) align 8, i8 noundef zeroext, \
             i16 noundef signext, ptr byval(%struct.S) align 8, \
             ptr noundef nonnull align 4 dereferenceable(16), \
             ptr nocapture readonly dereferenceable_or_null(8))\n",
            "declare noalias nonnull ptr @g(i1 zeroext)\n",
        ],
    );
}

/// §5: function attribute groups are deduplicated and numbered in first-use
/// order, with enum attributes before string attributes.
#[test]
fn attribute_groups() {
    let mut m = aarch64();
    let clang_fn = m.attr_set(&[
        Attr::string("target-features", "+neon"),
        Attr::Flag(AttrKind::NoInline),
        Attr::Flag(AttrKind::NoUnwind),
        Attr::Flag(AttrKind::OptimizeNone),
        Attr::UwTable(UwTableKind::Async),
        Attr::string("frame-pointer", "non-leaf"),
        Attr::string("no-trapping-math", "true"),
        Attr::string("stack-protector-buffer-size", "8"),
    ]);
    let decl_attrs = m.attr_set(&[
        Attr::Memory(MemoryEffects::argmem_only(ModRef::Read)),
        Attr::Flag(AttrKind::NoFree),
        Attr::Flag(AttrKind::WillReturn),
        Attr::AllocSize(0, None),
    ]);
    let mixed = m.attr_set(&[
        Attr::Memory(MemoryEffects {
            argmem: ModRef::ReadWrite,
            inaccessiblemem: ModRef::Write,
            other: ModRef::Read,
        }),
        Attr::AlignStack(16),
        Attr::VScaleRange(1, 16),
        Attr::AllocSize(0, Some(1)),
        Attr::string("empty", ""),
    ]);
    let vf = m.types.function(TypeId::VOID, &[TypeId::I64], false);
    let d = m.add_function("d", vf).unwrap();
    m.func_mut(d).attrs.func = decl_attrs;
    let vf2 = m.types.function(TypeId::VOID, &[TypeId::I64, TypeId::I64], false);
    let x = m.add_function("x", vf2).unwrap();
    m.func_mut(x).attrs.func = mixed;
    for name in ["a", "b"] {
        let mut b = define(&mut m, name, TypeId::VOID, &[TypeId::I64]);
        b.func().attrs.func = clang_fn;
        b.ret_void().unwrap();
    }
    check(
        &m,
        &[
            "declare void @d(i64) #0\n",
            "declare void @x(i64, i64) #1\n",
            "define dso_local void @a(i64 %0) #2 {\n",
            "define dso_local void @b(i64 %0) #2 {\n",
            "attributes #0 = { nofree willreturn allocsize(0) memory(argmem: read) }\n",
            // Enum, then type, then integer attributes (by LLVM enum value).
            "attributes #1 = { allocsize(0,1) \
             memory(read, argmem: readwrite, inaccessiblemem: write) alignstack=16 \
             vscale_range(1,16) \"empty\" }\n",
            "attributes #2 = { noinline nounwind optnone uwtable \"frame-pointer\"=\"non-leaf\" \
             \"no-trapping-math\"=\"true\" \"stack-protector-buffer-size\"=\"8\" \
             \"target-features\"=\"+neon\" }\n",
        ],
    );
}

/// §2.4: function-level properties.
#[test]
fn function_properties() {
    let mut m = aarch64();
    let pers_ty = m.types.function(TypeId::I32, &[], true);
    let pers = m.add_function("__gxx_personality_v0", pers_ty).unwrap();
    let mut b = define(&mut m, "props", TypeId::VOID, &[]);
    {
        let f = b.func();
        f.linkage = Linkage::Internal;
        f.dso_local = false;
        f.unnamed_addr = UnnamedAddr::Global;
        f.section = Some("text.hot".into());
        f.align = Some(16);
        f.gc = Some("statepoint-example".into());
        f.personality = Some(Value::func(pers));
        f.cc = CallConv::FAST;
    }
    b.ret_void().unwrap();
    let mut b = define(&mut m, "vis", TypeId::VOID, &[]);
    {
        let f = b.func();
        f.visibility = Visibility::Protected;
        f.unnamed_addr = UnnamedAddr::Local;
        f.dll_storage = DllStorage::Export;
    }
    b.ret_void().unwrap();
    check(
        &m,
        &[
            "define internal fastcc void @props() unnamed_addr section \"text.hot\" align 16 \
             gc \"statepoint-example\" personality ptr @__gxx_personality_v0 {\n",
            // Non-default visibility implies dso_local, which is then not printed.
            "define protected dllexport void @vis() local_unnamed_addr {\n",
        ],
    );
}

/// §2.4: calling-convention keywords, including the numeric `cc N` form.
#[test]
fn calling_conventions() {
    let mut m = aarch64();
    let vf = m.types.function(TypeId::VOID, &[], false);
    let ccs = [
        (CallConv::C, "declare void @cc0()\n"),
        (CallConv::FAST, "declare fastcc void @cc8()\n"),
        (CallConv::COLD, "declare coldcc void @cc9()\n"),
        (CallConv::PRESERVE_MOST, "declare preserve_mostcc void @cc14()\n"),
        (CallConv::PRESERVE_ALL, "declare preserve_allcc void @cc15()\n"),
        (CallConv::SWIFT, "declare swiftcc void @cc16()\n"),
        (CallConv::CXX_FAST_TLS, "declare cxx_fast_tlscc void @cc17()\n"),
        (CallConv::TAIL, "declare tailcc void @cc18()\n"),
        (CallConv::X86_64_SYSV, "declare x86_64_sysvcc void @cc78()\n"),
        (CallConv::WIN64, "declare win64cc void @cc79()\n"),
        (CallConv::AARCH64_VECTOR_PCS, "declare aarch64_vector_pcs void @cc97()\n"),
        (CallConv(200), "declare cc200 void @cc200()\n"),
    ];
    for (cc, _) in ccs {
        let f = m.add_function(&format!("cc{}", cc.0), vf).unwrap();
        m.func_mut(f).cc = cc;
    }
    let lines: Vec<&str> = ccs.iter().map(|c| c.1).collect();
    check(&m, &lines);
}
