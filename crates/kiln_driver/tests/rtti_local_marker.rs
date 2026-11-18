//! GCC's unit-local marker in `type_info` names (spec/abi.md "RTTI names
//! of unit-local types", spec/driver.md `-fkiln-rtti-local-marker`;
//! WP-1651): under libstdc++ (the default) the `_ZTS` string of a type
//! local to the unit starts with `*`, where GCC 16.1 writes one; with
//! `-stdlib=libc++` nothing changes; the switch pair forces it either way,
//! the last one winning; C and `-fno-rtti` follow the same rule (C has no
//! `type_info`; `-fno-rtti` still emits an exception's, marked as GCC
//! does). The IR differs from clang's in the marker only, so this checks
//! the strings (`tests/codegen/rtti_local_names.cpp` compares the rest with
//! clang). No header and no pinned target: the source is target-neutral.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

const SOURCE: &str = r#"namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std
const std::type_info *sink[64];
int n;
#define T(x) sink[n++] = &typeid(x)
namespace {
struct A {};
enum EA { ea };
struct IncA;
} // namespace
static void sfun() { struct LS {}; T(LS); auto l = [] {}; T(l); }
inline void ifun() { struct LI {}; T(LI); auto l = [] {}; T(l); }
void efun() { struct LE {}; T(LE); }
template <class U> void tf() { struct L {}; T(L); }
auto nl = [] {};
static auto snl = [] {};
struct { int a; } us;
template <class> struct Tm {};
template <int *> struct Tp {};
static int sint;
int eint;
struct Ext {};
struct DerivA : A {};
struct Inc;
struct [[gnu::visibility("hidden")]] Hid {};
struct [[clang::internal_linkage]] IL {};
void use() {
  sfun(); ifun(); efun(); tf<int>(); tf<A>();
  T(A); T(EA); T(nl); T(snl); T(us);
  T(Tm<A>); T(Tp<&sint>); T(Tp<&eint>); T(Tm<Ext>);
  T(A *); T(int A::*); T(Ext A::*); T(void(A)); T(A[3]); T(const A *);
  T(DerivA); T(Ext); T(Inc *); T(IncA *); T(int Inc::*); T(Hid); T(IL);
}
"#;

/// `(@ symbol of the name, marked)`: GCC 16.1's marker for the form. A
/// `$_N` class outside a function is marked only as the closure of a
/// `static` (`const`, unnamed-namespace) variable's initializer or an
/// unnamed class at namespace scope, an incomplete class only in an
/// unnamed namespace (WP-1679); clang's `internal_linkage` is left
/// unmarked whatever GCC does: the rule's conservative side.
const FORMS: &[(&str, bool)] = &[
    ("_ZTSN12_GLOBAL__N_11AE", true),
    ("_ZTSN12_GLOBAL__N_12EAE", true),
    ("_ZTSZL4sfunvE2LS", true),
    ("\"_ZTSZL4sfunvE3$_0\"", true),
    ("_ZTSZ4ifunvE2LI", false),
    ("_ZTSZ4ifunvEUlvE_", false),
    ("_ZTSZ4efunvE2LE", true),
    ("_ZTSZ2tfIiEvvE1L", false),
    ("_ZTSZ2tfIN12_GLOBAL__N_11AEEvvE1L", true),
    ("_ZTS2TmIN12_GLOBAL__N_11AEE", true),
    ("_ZTS2TpIXadL_ZL4sintEEE", true),
    ("_ZTS2TpIXadL_Z4eintEEE", false),
    ("_ZTS2TmI3ExtE", false),
    ("_ZTSPN12_GLOBAL__N_11AE", true),
    ("_ZTSMN12_GLOBAL__N_11AEi", true),
    ("_ZTSMN12_GLOBAL__N_11AE3Ext", true),
    ("_ZTSFvN12_GLOBAL__N_11AEE", true),
    ("_ZTSA3_N12_GLOBAL__N_11AE", true),
    ("_ZTSPKN12_GLOBAL__N_11AE", true),
    ("_ZTS6DerivA", false),
    ("_ZTS3Ext", false),
    ("_ZTSP3Inc", false),
    ("_ZTSPN12_GLOBAL__N_14IncAE", true),
    ("_ZTSN12_GLOBAL__N_14IncAE", true),
    ("\"_ZTS3$_0\"", false),
    ("\"_ZTS3$_1\"", true),
    ("\"_ZTS3$_2\"", true),
    ("_ZTSM3Inci", false),
    ("_ZTS3Hid", false),
    ("_ZTSL2IL", false),
];

fn ir(name: &str, src: &str, file: &str, args: &[&str]) -> String {
    let d = std::env::temp_dir().join(format!("kiln-rtti-marker-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(file), src).unwrap();
    let o = Command::new(kilnpp())
        .args(["-w", "-S", "-emit-llvm", "-o", "-"])
        .args(args)
        .arg(file)
        .current_dir(&d)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

/// The string of `@<sym>` (`sym` without the `@`, quoted if LLVM quotes
/// it).
fn name_string<'a>(ir: &'a str, sym: &str) -> &'a str {
    let head = format!("@{sym} ");
    let line =
        ir.lines().find(|l| l.starts_with(&head)).unwrap_or_else(|| panic!("no {head} in:\n{ir}"));
    let s = &line[line.find("c\"").expect("a string") + 2..];
    &s[..s.find("\\00\"").expect("NUL-terminated")]
}

fn marked(ir: &str) -> usize {
    ir.lines().filter(|l| l.starts_with('@') && l.contains("_ZTS") && l.contains("c\"*")).count()
}

/// [expr.typeid], [basic.link]: every form of the table, under the
/// default library (libstdc++) and when forced on with libc++.
#[test]
fn marker_where_gcc_writes_it() {
    for (name, args) in [
        ("default", &[][..]),
        ("libstdcxx", &["-stdlib=libstdc++"][..]),
        ("forced", &["-stdlib=libc++", "-fkiln-rtti-local-marker"][..]),
    ] {
        let ir = ir(name, SOURCE, "t.cpp", args);
        for &(sym, mark) in FORMS {
            let s = name_string(&ir, sym);
            let plain = sym.trim_matches('"').trim_start_matches("_ZTS");
            let want = if mark { format!("*{plain}") } else { plain.to_owned() };
            assert_eq!(s, want, "{sym} under {args:?}");
        }
        assert_eq!(marked(&ir), FORMS.iter().filter(|f| f.1).count(), "{args:?}:\n{ir}");
    }
}

/// libc++ compares `type_info`s by address and would print the `*` in
/// `name()`: nothing is marked, and the IR is the switched-off IR byte
/// for byte; the last switch wins.
#[test]
fn no_marker_with_libcxx_or_switched_off() {
    let off = ir("off", SOURCE, "t.cpp", &["-fno-kiln-rtti-local-marker"]);
    assert_eq!(marked(&off), 0, "{off}");
    let libcxx = ir("libcxx", SOURCE, "t.cpp", &["-stdlib=libc++"]);
    assert_eq!(libcxx, off);
    let last =
        ir("last", SOURCE, "t.cpp", &["-fkiln-rtti-local-marker", "-fno-kiln-rtti-local-marker"]);
    assert_eq!(last, off);
    let on =
        ir("on", SOURCE, "t.cpp", &["-fno-kiln-rtti-local-marker", "-fkiln-rtti-local-marker"]);
    assert_eq!(marked(&on), FORMS.iter().filter(|f| f.1).count());
}

/// [except.throw]: under `-fno-rtti` an exception's `type_info` is still
/// emitted, and marked as GCC 16.1 marks it.
#[test]
fn fno_rtti_exception_type_info() {
    let src = "namespace { struct S {}; }\nvoid f() { throw S(); }\n";
    let ir = ir("nortti", src, "t.cpp", &["-fno-rtti"]);
    assert_eq!(name_string(&ir, "_ZTSN12_GLOBAL__N_11SE"), "*N12_GLOBAL__N_11SE");
}

/// C has no `type_info`: the switch changes nothing.
#[test]
fn c_unchanged() {
    let src = "static struct { int a; } s;\nint *f(void) { return &s.a; }\n";
    let plain = ir("c-plain", src, "t.c", &[]);
    assert_eq!(ir("c-on", src, "t.c", &["-fkiln-rtti-local-marker"]), plain);
    assert!(!plain.contains("_ZTS"));
}
