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
