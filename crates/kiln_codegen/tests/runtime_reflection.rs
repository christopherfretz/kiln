//! Code generation of variables that hold reflections, decided by value
//! (P4101R1; spec/codegen.md "Consteval-only variables" and "Run-time
//! reflections", WP-1328, WP-1361). An immediate object has no storage;
//! an object whose reflections are all null exists at run time, its
//! `std::meta::info` a pointer-sized null (`ptr`, passed and returned as
//! `std::nullptr_t` is on both targets). A pointer to an immediate
//! function in an immediate object is never emitted (P3603). kiln only:
//! clang 19/22 lack `^^`.

use std::sync::Arc;

use kiln_abi::{Arch, Target};
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const PRELUDE: &str = "using info = decltype(^^int);\n\
                       struct S { info r; const char* name; int n; };\n";

/// Parses `PRELUDE` + `src` with reflection (no sema error allowed) and
/// lowers it: the IR, or codegen's error message.
fn lower(src: &str) -> Result<String, String> {
    lower_for(src, Arch::AArch64)
}

/// [`lower`] for target `arch`.
fn lower_for(src: &str, arch: Arch) -> Result<String, String> {
    let sm = SourceManager::new();
    let text = format!("{PRELUDE}{src}");
    let file = sm.add_virtual("t.cpp", text).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(arch);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions { reflection: true, ..ParseOptions::default() },
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "sema errors: {errors:?}");
    let opts = CodegenOptions { source_filename: "t.cpp".to_owned(), ..CodegenOptions::default() };
    emit(&mut tu.ast, &mut tu.oven, &target, &opts).map(|m| m.to_string()).map_err(|e| e.msg)
}

/// The IR line of the definition of the function whose mangled name is
/// `name`.
fn define_line<'a>(ir: &'a str, name: &str) -> &'a str {
    let at = format!("@{name}(");
    ir.lines()
        .find(|l| l.starts_with("define") && l.contains(&at))
        .unwrap_or_else(|| panic!("no definition of {name}:\n{ir}"))
}

#[test]
fn null_reflection_objects_exist_at_run_time() {
    let src = "info g;\n\
               S gs;\n\
               info* gp;\n\
               int f() { info x; static info y; S s{{}, \"x\", 1}; (void)x; (void)y; return s.n; }\n";
    for arch in [Arch::AArch64, Arch::X86_64] {
        let ir = lower_for(src, arch).expect("codegen");
        // Zero-initialized: the null reflection is a null pointer.
        assert!(ir.contains("@g = dso_local global ptr null"), "{arch:?} g:\n{ir}");
        assert!(
            ir.contains("@gs = dso_local global %struct.S zeroinitializer"),
            "{arch:?} gs:\n{ir}"
        );
        assert!(ir.contains("@gp = dso_local global ptr null"), "{arch:?} gp:\n{ir}");
        assert!(ir.contains("alloca ptr"), "{arch:?} local info:\n{ir}");
    }
}

#[test]
fn run_time_info_is_passed_as_nullptr_t() {
    // `std::meta::info` is passed and returned exactly as
    // `std::nullptr_t`: a `ptr` in a general-purpose register (x86-64
    // INTEGER class, AArch64 `x` register), without `noundef`; a class
    // holding one by its members' classification.
    let src = "info id(info x) { return x; }\n\
               decltype(nullptr) nid(decltype(nullptr) x) { return x; }\n\
               bool eq(info a, info b) { return a == b; }\n\
               struct P { info a; info b; };\n\
               P idp(P p) { return p; }\n";
    for arch in [Arch::AArch64, Arch::X86_64] {
        let ir = lower_for(src, arch).expect("codegen");
        let info_sig = define_line(&ir, "_Z2idDm");
        assert!(info_sig.starts_with("define dso_local ptr @_Z2idDm(ptr "), "{arch:?}: {info_sig}");
        let null_sig = define_line(&ir, "_Z3nidDn");
        assert_eq!(
            info_sig.replace("_Z2idDm", "F"),
            null_sig.replace("_Z3nidDn", "F"),
            "{arch:?}: info and nullptr_t differ"
        );
        assert!(ir.contains("icmp eq ptr"), "{arch:?}: info ==:\n{ir}");
        let pair = define_line(&ir, "_Z3idp1P");
        let want = match arch {
            Arch::AArch64 => "define dso_local [2 x i64] @_Z3idp1P([2 x i64] ",
            Arch::X86_64 => "define dso_local { ptr, ptr } @_Z3idp1P(ptr ",
        };
        assert!(pair.starts_with(want), "{arch:?}: {pair}");
    }
}

#[test]
fn immediate_function_pointers_are_never_emitted() {
    // P3603 (carried by P4101R1): a `constexpr` variable or template
    // argument holding a pointer to an immediate function is an immediate
    // object; nothing refers to the function at run time.
    let ir = lower(
        "consteval int plus1(int x) { return x + 1; }\n\
         template <auto V> struct C { static constexpr int v = V(1); };\n\
         constexpr auto b = plus1;\n\
         constexpr int (*arr[])(int) = {plus1, plus1};\n\
         static_assert(b(1) == 2 && arr[1](2) == 3);\n\
         auto c = C<plus1>();\n\
         int f() { return C<plus1>::v + c.v; }\n",
    )
    .expect("codegen");
    for bad in ["plus1i(", "@_Z5plus1i", "@_ZL1b", "@_ZL3arr", "@b ", "@arr "] {
        assert!(!ir.contains(bad), "emitted {bad}:\n{ir}");
    }
}

#[test]
fn immediate_objects_get_no_storage() {
    let ir = lower(
        "constexpr info r = ^^int;\n\
         constexpr S gs{^^int, \"g\", 1};\n\
         extern constexpr S xs{^^char, \"x\", 2};\n\
         int f() { constexpr S ls{^^long, \"l\", 3}; constexpr int n = ls.n + xs.n; return n; }\n",
    )
    .expect("codegen");
    for name in ["@r ", "@gs ", "@xs ", "@_ZL2gs", "@_ZL1r"] {
        assert!(!ir.contains(name), "emitted {name}:\n{ir}");
    }
}

#[test]
fn constexpr_null_holders_are_ordinary_objects() {
    // `gs` holds a null reflection: not an immediate object, so it is an
    // ordinary constant variable, read (or folded) at run time.
    let ir = lower(
        "constexpr S gs{{}, \"g\", 1};\n\
         const S* addr() { return &gs; }\n\
         const char* name() { return gs.name; }\n\
         int num() { constexpr S ls{{}, \"l\", 2}; (void)ls; return ls.n + gs.n; }\n",
    )
    .expect("codegen");
    assert!(ir.contains("@_ZL2gs = internal constant %struct.S { ptr null"), "gs:\n{ir}");
}

/// The lines of the definition of `name`, from its `define` to the closing
/// `}`.
fn body<'a>(ir: &'a str, name: &str) -> Vec<&'a str> {
    let head = define_line(ir, name);
    ir.lines().skip_while(|l| *l != head).take_while(|l| *l != "}").collect()
}

#[test]
fn constexpr_locals_keep_storage_unless_immediate() {
    // spec/codegen.md "Constexpr locals" (WP-1410): a block-scope
    // `constexpr` local holding only null reflections is an ordinary
    // automatic object, so at -O0 it has storage even when only consteval
    // calls read it (clang's rule for every local). One holding a non-null
    // reflection is an immediate object: no `alloca`, no `@__const`, in a
    // plain function, a function template instance (the reads in a
    // designated initializer) and a template whose parameter is a
    // reflection.
    let src = "consteval S mk(bool nn) { S s{}; s.n = 3; if (nn) s.r = ^^int; return s; }\n\
               consteval int num(const S& s) { return s.n; }\n\
               struct O { int x; int y; };\n\
               int null_holder() { constexpr S s = mk(false); return num(s); }\n\
               int immediate() { constexpr S s = mk(true); return num(s); }\n\
               template <class T> O designated() {\n\
                 constexpr S s = mk(true); return O{.x = num(s), .y = num(s) + 1}; }\n\
               template <info R> int by_refl() { constexpr S s = mk(R != info{}); return num(s); }\n\
               int use() { return designated<int>().y + by_refl<^^int>(); }\n";
    for arch in [Arch::AArch64, Arch::X86_64] {
        let ir = lower_for(src, arch).expect("codegen");
        let null = body(&ir, "_Z11null_holderv");
        assert!(null.iter().any(|l| l.contains("alloca %struct.S")), "{arch:?} null_holder:\n{ir}");
        assert!(ir.contains("@__const._Z11null_holderv.s = "), "{arch:?} null_holder:\n{ir}");
        for f in ["_Z9immediatev", "_Z10designatedIiE1Ov", "_Z7by_reflILDmtyiEEiv"] {
            let b = body(&ir, f);
            assert!(!b.iter().any(|l| l.contains("alloca %struct.S")), "{arch:?} {f}:\n{ir}");
            assert!(!ir.contains(&format!("@__const.{f}")), "{arch:?} {f}:\n{ir}");
        }
    }
}
