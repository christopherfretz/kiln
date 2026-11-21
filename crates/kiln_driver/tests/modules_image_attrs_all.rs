//! WP-1806: every attribute kind the declaration image carries since
//! WP-1806 (spec/modules.md §11.11 "Attributes"). An importer that builds
//! a module from its image emits the same IR as one that replays it, for
//! target-neutral, x86-64 and `AArch64` attributes, and the effects are
//! clang's (checked by hand against clang++-22). A precompiled header
//! of the same declarations restores them from its snapshot with the
//! IR of the replayed header.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-bmi-attrs-all-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs kiln++ in `dir`: exit code and stderr.
fn kiln(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(kilnpp())
        .arg("-std=c++26")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    (o.status.code().unwrap_or(-1), stderr)
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let (code, stderr) = kiln(dir, args);
    assert_eq!(code, 0, "kiln++ {args:?}:\n{stderr}");
    stderr
}

const AM: &str = "export module a;\nexport {\n#include \"a.h\"\n}\n";

/// Target-neutral attributes.
const NEUTRAL_H: &str = r#"extern "C" int wa_ov(int) __attribute__((overloadable));
extern "C" int wd_real(int x) { return x; }
extern "C" void *wd_resolve() { return (void *)wd_real; }
namespace wa {
inline void done(int *p) { *p = 0; }
__attribute__((section(".text.wa"))) inline int in_sec(int x) { return x + 1; }
__attribute__((min_vector_width(128))) inline int mvw(int x) { return x + 2; }
__attribute__((constructor(200))) void ctor_fn();
__attribute__((destructor)) void dtor_fn();
inline int clean(int x) {
  int v __attribute__((cleanup(done))) = x;
  return v;
}
inline int uninit(int x) {
  [[clang::uninitialized]] int buf[4];
  buf[0] = x;
  return buf[0];
}
inline unsigned long osz(void *const p __attribute__((pass_object_size(0)))) {
  return __builtin_object_size(p, 0);
}
struct __attribute__((ms_struct)) MS { char a; int b : 4; char c; };
__attribute__((no_builtin("memcpy"))) inline int nob(int x) { return x + 3; }
__attribute__((patchable_function_entry(2, 1))) inline int pfe(int x) { return x + 4; }
__attribute__((speculative_load_hardening)) inline int slh(int x) { return x + 5; }
[[clang::xray_always_instrument]] inline int xa(int x) { return x + 7; }
[[clang::xray_log_args(1)]] inline int xl(int x) { return x + 8; }
__attribute__((assume_aligned(32))) void *aligned_ptr();
typedef double *__attribute__((align_value(64))) aligned_double;
inline double first(aligned_double p) { return *p; }
void cb(void (*f)(int), int x) __attribute__((callback(f, x)));
__attribute__((annotate("wa_note", 3))) inline int ann(int x) { return x + 9; }
#pragma clang section text = ".wa_text"
inline int psec(int x) { return x + 10; }
#pragma clang section text = ""
extern int wi __attribute__((weak_import));
[[clang::loader_uninitialized]] inline int lu;
int alias_of(int) __attribute__((alias("wd_real")));
int via_ifunc(int) __attribute__((ifunc("wd_resolve")));
__attribute__((selectany)) int sel = 5;
inline int indet(int x [[indeterminate]]) { int y [[indeterminate]]; y = x; return y; }
inline int mu(int x __attribute__((maybe_undef))) { return x; }
}
"#;

const NEUTRAL_U: &str = r"static void sink(int) {}
int use() {
  char buf[10];
  double d = 2;
  int z = 1;
  wa::cb(sink, 1);
  return wa::in_sec(1) + wa::mvw(2) + wa::clean(3) + wa::uninit(4) + (int)wa::osz(buf)
       + (int)sizeof(wa::MS) + wa::nob(5) + wa::pfe(6) + wa::slh(7) + wa::xa(9) + wa::xl(10)
       + (wa::aligned_ptr() != nullptr) + (int)wa::first(&d) + wa::ann(11) + wa::psec(12)
       + (&wa::wi != nullptr) + wa_ov(1) + wa::lu + wa::alias_of(1) + wa::via_ifunc(2)
       + wa::sel + wa::indet(4) + wa::mu(z);
}
";

/// x86-64 attributes (`--target=x86_64-unknown-linux-gnu`).
const X86_H: &str = r#"namespace wb {
__attribute__((model("large"))) extern int big;
__attribute__((nocf_check)) inline int ncf(int x) { return x + 1; }
__attribute__((function_return("thunk-extern"))) inline int fret(int x) { return x + 2; }
__attribute__((force_align_arg_pointer)) inline int fal(int x) { return x + 3; }
__attribute__((zero_call_used_regs("used-gpr"))) inline int zc(int x) { return x + 4; }
__attribute__((target("avx2"))) inline int tgt(int x) { return x + 5; }
__attribute__((min_vector_width(256))) inline int mvw(int x) { return x + 6; }
__attribute__((target_clones("avx2", "default"))) inline int tcl(int x) { return x + 7; }
}
"#;

const X86_U: &str = "int use() {\n\
                     \x20 return wb::big + wb::ncf(1) + wb::fret(2) + wb::fal(3) + wb::zc(4)\n\
                     \x20      + wb::tgt(5) + wb::mvw(6) + wb::tcl(7);\n\
                     }\n";

/// `AArch64` attributes (`--target=aarch64-unknown-linux-gnu`), with a
/// Swift parameter ABI in function types.
const A64_H: &str = r#"namespace wc {
__arm_locally_streaming inline int ls(int x) { return x + 1; }
__arm_new("za") inline int nz(int x) { return x + 2; }
__attribute__((target_version("default"))) inline int tv(int x) { return x + 3; }
__attribute__((target("crc"))) inline int tc(int x) { return x + 4; }
__attribute__((swiftcall)) inline int sw(int x, void *ctx __attribute__((swift_context))) { return x + 1; }
__attribute__((swiftcall)) int swe(void *ctx __attribute__((swift_context)), void **err __attribute__((swift_error_result)));
using fp = decltype(&sw);
inline fp get() { return sw; }
template <class T> __attribute__((swiftcall)) T tsw(T x, void *c __attribute__((swift_context))) { return x; }
}
"#;

const A64_U: &str = "int use() {\n\
                     \x20 void *e = nullptr;\n\
                     \x20 wc::fp f = wc::get();\n\
                     \x20 return wc::ls(1) + wc::nz(2) + wc::tv(3) + wc::tc(4) + wc::sw(1, nullptr)\n\
                     \x20      + wc::swe(nullptr, &e) + f(2, nullptr) + wc::tsw<long>(4, nullptr);\n\
                     }\n";

/// Builds module `a` from header `h` and compiles `use` against it from
/// the image and by replay: the image is used, and the IR is the same.
fn image_ir(name: &str, h: &str, uses: &str, flags: &[&str]) -> String {
    let d = scratch(name);
    std::fs::write(d.join("a.h"), h).unwrap();
    std::fs::write(d.join("a.cppm"), AM).unwrap();
    std::fs::write(d.join("u.cpp"), format!("import a;\n{uses}")).unwrap();
    let pre =
        [flags, &["-v", "-fkiln-bmi-image", "--precompile", "a.cppm", "-o", "a.pcm"]].concat();
    let v = ok(&d, &pre);
    assert!(v.contains("with a declaration image of "), "{v}");
    assert!(!v.contains("no declaration image"), "{v}");
    let ir_args =
        [flags, &["-fmodule-file=a=a.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]].concat();
    let v = ok(&d, &[&["-v", "-fkiln-bmi-image"][..], &ir_args].concat());
    assert!(
        v.lines().any(|l| l == "note: imported 'a' from its declaration image"),
        "the image is used:\n{v}"
    );
    let img = std::fs::read_to_string(d.join("u.ll")).unwrap();
    ok(&d, &[&["-fno-kiln-bmi-image"][..], &ir_args].concat());
    let rep = std::fs::read_to_string(d.join("u.ll")).unwrap();
    assert!(img == rep, "IR differs between the image and the replay:\n{img}\n----\n{rep}");
    let _ = std::fs::remove_dir_all(&d);
    img
}

fn has_all(ir: &str, wants: &[&str]) {
    for want in wants {
        assert!(ir.contains(want), "{want}\n{ir}");
    }
}

#[test]
fn neutral_attributes_behave_as_replayed() {
    let ir = image_ir("neutral", NEUTRAL_H, NEUTRAL_U, &["-Wno-everything"]);
    has_all(
        &ir,
        &[
            "@_ZN2waW1a2wiE = extern_weak global i32",
            "@_ZN2waW1a2luE = linkonce_odr dso_local global i32 undef",
            "c\"wa_note\\00\", section \"llvm.metadata\"",
            "@llvm.global.annotations",
            "declare !callback ",
            "section \".text.wa\" comdat",
            "section \".wa_text\" comdat",
            "@_ZN2waW1a3oszEPvU17pass_object_size0(ptr noundef %0, i64 noundef %1)",
            "@_ZN2waW1a5firstEPd(ptr noundef align 64 %0)",
            "declare i32 @_Z5wa_ovi(i32 noundef)",
            "declare noundef i32 @_ZN2waW1a8alias_ofEi(i32 noundef)",
            "declare noundef i32 @_ZN2waW1a9via_ifuncEi(i32 noundef)",
            "@_ZN2waW1a3selE = external global i32",
            "\"no-builtin-memcpy\"",
            "\"patchable-function-entry\"=\"1\"",
            "\"patchable-function-prefix\"=\"1\"",
            "speculative_load_hardening",
        ],
    );
    // The xray attributes under -fxray-instrument; the trivial
    // auto-var-init exemptions under =pattern.
    let ir = image_ir(
        "neutral-xray",
        NEUTRAL_H,
        NEUTRAL_U,
        &["-Wno-everything", "-fxray-instrument", "-ftrivial-auto-var-init=pattern"],
    );
    has_all(&ir, &["\"function-instrument\"=\"xray-always\"", "\"xray-instruction-threshold\""]);
}

#[test]
fn x86_attributes_behave_as_replayed() {
    let ir = image_ir(
        "x86",
        X86_H,
        X86_U,
        &["--target=x86_64-unknown-linux-gnu", "-fcf-protection", "-Wno-everything"],
    );
    has_all(
        &ir,
        &[
            "@_ZN2wbW1a3bigE = external global i32, code_model \"large\"",
            "@_ZN2wbW1a3tclEi = weak_odr dso_local ifunc i32 (i32), ptr @_ZN2wbW1a3tclEi.resolver",
            "@_ZN2wbW1a3tclEi.avx2.0(",
            "\"min-legal-vector-width\"=\"256\"",
            "\"stackrealign\"",
            "\"zero-call-used-regs\"=\"used-gpr\"",
            "fn_ret_thunk_extern",
            "nocf_check",
            "+avx2",
        ],
    );
}

#[test]
fn aarch64_attributes_behave_as_replayed() {
    let ir = image_ir(
        "aarch64",
        A64_H,
        A64_U,
        &["--target=aarch64-unknown-linux-gnu", "-march=armv9-a+sme", "-Wno-everything"],
    );
    has_all(
        &ir,
        &[
            "\"aarch64_pstate_sm_body\"",
            "\"aarch64_new_za\"",
            "\"fmv-features\"",
            "+crc",
            "define linkonce_odr dso_local swiftcc noundef i32 @_ZN2wcW1a2swEiPv(i32 noundef %0, ptr noundef swiftself %1)",
            "declare swiftcc noundef i32 @_ZN2wcW1a3sweEPvPS1_(ptr noundef swiftself, ptr noundef swifterror)",
            "@_ZN2wcW1a3tswIlEET_S2_Pv(i64 noundef %0, ptr noundef swiftself %1)",
        ],
    );
}

/// The same declarations in a precompiled header: restored from the
/// snapshot, the IR is the replayed header's.
#[test]
fn pch_attributes_behave_as_replayed() {
    for (name, h, uses, flags) in [
        ("pch-neutral", NEUTRAL_H, NEUTRAL_U, &["-Wno-everything"][..]),
        (
            "pch-x86",
            X86_H,
            X86_U,
            &["--target=x86_64-unknown-linux-gnu", "-fcf-protection", "-Wno-everything"],
        ),
        (
            "pch-aarch64",
            A64_H,
            A64_U,
            &["--target=aarch64-unknown-linux-gnu", "-march=armv9-a+sme", "-Wno-everything"],
        ),
    ] {
        let d = scratch(name);
        std::fs::write(d.join("a.h"), h).unwrap();
        std::fs::write(d.join("u.cpp"), uses).unwrap();
        ok(
            &d,
            &[flags, &["-fkiln-pch-snapshot", "-x", "c++-header", "a.h", "-o", "a.pch"]].concat(),
        );
        let ir_args =
            [flags, &["-include-pch", "a.pch", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]].concat();
        let v = ok(&d, &[&["-v", "-fkiln-pch-snapshot"][..], &ir_args].concat());
        assert!(
            v.contains("': snapshot matches; restoring it"),
            "{name}: the snapshot is used:\n{v}"
        );
        let snap = std::fs::read_to_string(d.join("u.ll")).unwrap();
        ok(&d, &[&["-fno-kiln-pch-snapshot"][..], &ir_args].concat());
        let rep = std::fs::read_to_string(d.join("u.ll")).unwrap();
        assert!(snap == rep, "{name}: IR differs between the snapshot and the replay");
        let _ = std::fs::remove_dir_all(&d);
    }
}
