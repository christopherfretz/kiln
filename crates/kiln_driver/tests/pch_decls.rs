//! WP-1036: the PCH's declaration image end to end through `kiln++`
//! (spec/driver.md "Precompiled headers"; the image is spec/modules.md
//! §11.11). With `-fkiln-pch-image=load` a compile installs the
//! preprocessor-state record and builds the header's declarations from
//! the image instead of replaying it, and must behave as the `#include`
//! does ([cpp.include]/2): the same diagnostics, byte for byte, and the
//! same IR, including the header's definitions the unit odr-uses
//! ([basic.def.odr]). A header the image cannot carry is replayed, with
//! the reason under `-v`; `check` runs both and compares them.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pch-decls-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    let o =
        Command::new(kilnpp()).args(args).current_dir(dir).stdin(Stdio::null()).output().unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    }
}

fn ok(dir: &Path, args: &[&str]) -> Out {
    let o = kiln(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    o
}

/// Builds `<name>.pch` from `<name>.h` (with the record and the image),
/// returning the producer's `-v` note.
fn produce(dir: &Path, name: &str, header: &str) -> String {
    std::fs::write(dir.join(format!("{name}.h")), header).unwrap();
    let (h, pch) = (format!("{name}.h"), format!("{name}.pch"));
    let o = ok(dir, &["-v", "-fkiln-pch-image", "-x", "c++-header", &h, "-o", &pch]);
    o.stderr.lines().filter(|l| l.starts_with("note: precompiled header")).collect()
}

/// Compiles `tu` with the PCH replayed and with the image, and asserts
/// the exit code, the diagnostics and the output are the same; returns
/// the image run's.
fn same(dir: &Path, pch: &str, mode: &[&str], tu: &str) -> Out {
    let replay = kiln(dir, &[&["-include-pch", pch][..], mode, &[tu, "-o", "-"]].concat());
    let load = ["-fkiln-pch-image=load", "-include-pch", pch];
    let image = kiln(dir, &[&load[..], mode, &[tu, "-o", "-"]].concat());
    assert_eq!(replay.code, image.code, "{mode:?}:\n{}\n---\n{}", replay.stderr, image.stderr);
    assert_eq!(replay.stderr, image.stderr, "{mode:?}");
    assert_eq!(replay.stdout, image.stdout, "{mode:?}");
    image
}

/// The `-v` note of a consumer of `pch`.
fn path_note(dir: &Path, pch: &str, tu: &str) -> String {
    let o = kiln(dir, &["-v", "-fkiln-pch-image=load", "-include-pch", pch, "-fsyntax-only", tu]);
    o.stderr.lines().filter(|l| l.starts_with("note: precompiled header")).collect()
}

const RICH: &str = "\
template <class T> struct Box { T v; T get() const { return v; } static int count; };
template <class T> int Box<T>::count = 3;
template <class T> constexpr T sq(T t) { return t * t; }
template <class T> struct TB { static T make() { return T{}; } };
struct Base { virtual ~Base(); virtual int f() const { return 1; } };
struct Der : Base { int f() const override { return 2; } };
enum class Col { R, G = 5 };
enum E : unsigned char { E0, E1 = 200 };
typedef unsigned U;
using V = Box<int>;
namespace n { struct A {}; inline int adl(A) { return 7; } }
namespace outer { inline namespace v1 { int inl() { return 2; } } }
namespace al = outer;
using outer::inl;
static int st(int x) { return x; }
inline int iv = 4;
extern int ev;
int tentative;
static int internal = 6;
constexpr int K = sq(3);
int wrong(int);
struct SD { static int x; static const int y = 3; int bf : 3; };
int SD::x = 11;
inline int counter() { static int n = 0; return ++n; }
struct Op { int v; Op operator+(Op o) const { return {v + o.v}; } };
inline bool operator==(Op a, Op b) { return a.v == b.v; }
extern \"C\" int cfun(int);
extern \"C\" inline int cinl(int a) { return a; }
struct Del { Del() = default; Del(const Del &) = delete; };
int defarg(int a, int b = 4) { return a + b; }
const char *str() { return \"hello\"; }
struct Nest { struct In { int q = 5; }; In in; };
struct Agg { int a, b; };
constexpr Agg agg{1, 2};
int over(int);
int over(double, double);
";

/// The unit redeclares, defines and specializes what the header
/// declares, and uses its classes, templates, overloads and ADL.
const RICH_TU: &str = "\
Base::~Base() {}
int ev = 2;
extern int tentative;
int wrong(int);
template <> struct Box<char> { int z = 9; };
static_assert(sq(4) == 16 && K == 9);
int cfun(int a) { return a * 2; }
int over(int a) { return a; }
int main() {
  Box<int> b{5}; V v{6}; Box<char> c; Der d; const Base &r = d;
  SD s{}; s.bf = 2; Op x{1}, y{2}; Del del; (void)del;
  return b.get() + v.get() + c.z + r.f() + (int)Col::G + U(1) + adl(n::A{}) + st(1) + iv + ev
       + Box<int>::count + SD::x + SD::y + s.bf + counter() + (x + y == Op{3}) + al::inl()
       + inl() + cfun(1) + cinl(1) + E1 + defarg(1) + str()[1] + Nest{}.in.q
       + TB<int>::make() + agg.b + over(1) + tentative + internal;
}
";

/// A compile with the image matches replay: diagnostics, the IR (every
/// definition from the header the unit uses is emitted) and an object.
#[test]
fn image_matches_replay() {
    let d = scratch("rich");
    let note = produce(&d, "r", RICH);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    std::fs::write(d.join("r.cpp"), RICH_TU).unwrap();
    assert!(path_note(&d, "r.pch", "r.cpp").contains("': declaration image ("));
    same(&d, "r.pch", &["-fsyntax-only", "-Wall"], "r.cpp");
    let ir = same(&d, "r.pch", &["-S", "-emit-llvm", "-O0"], "r.cpp");
    for def in ["@_Z6defargii(", "@_Z3strv(", "@_ZN5outer2v13inlEv(", "@_ZL2sti("] {
        assert!(ir.stdout.lines().any(|l| l.starts_with("define") && l.contains(def)), "{def}");
    }
    for var in ["@_ZN2SD1xE = ", "@tentative = ", "@_ZL8internal = ", "@ev = "] {
        assert!(ir.stdout.contains(var), "{var}");
    }
    same(&d, "r.pch", &["-S", "-emit-llvm", "-O1"], "r.cpp");
}

/// Errors and notes into the header (overload candidates, a mismatched
/// redeclaration, an instantiation through a nested header) print the
/// same carets and include stacks as replay.
#[test]
fn diagnostics_into_the_header() {
    let d = scratch("diags");
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("sub/deep.h"), "template <class T> void deep(T t) { int *p = t; }\n")
        .unwrap();
    let header = "\
#include \"sub/deep.h\"
struct S { int m; void g(int); };
int over(int);
int over(double, double);
template <class T> void needs_int(T t) { int *p = t; }
long redecl(int);
";
    produce(&d, "b", header);
    std::fs::write(
        d.join("b.cpp"),
        "int x = over(\"a\");\nvoid u() { needs_int(1.0); deep(1.0); }\nint redecl(int);\n\
         void S::g(char) {}\n",
    )
    .unwrap();
    let o = same(&d, "b.pch", &["-fsyntax-only"], "b.cpp");
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("In file included from <command line>:"), "{}", o.stderr);
    assert!(o.stderr.contains("./sub/deep.h:1:"), "{}", o.stderr);
    assert!(o.stderr.contains("./b.h:3:5: note: candidate function"), "{}", o.stderr);
}

/// The header's open `#pragma pack` and `clang diagnostic` pushes apply
/// after it, from the record ([cpp.pragma]).
#[test]
fn pragma_state_crosses_the_boundary() {
    let d = scratch("pragmas");
    let header = "struct P { char c; int i; };\n#pragma pack(push, 1)\n\
                  #pragma clang diagnostic push\n#pragma clang diagnostic ignored \"-Wunused-value\"\n";
    assert!(produce(&d, "p", header).contains("declaration image"));
    let tu = "\
struct Q { char c; int i; };
static_assert(sizeof(P) == 2 * sizeof(int));
static_assert(sizeof(Q) == 1 + sizeof(int));
#pragma pack(pop)
struct R { char c; int i; };
static_assert(sizeof(R) == 2 * sizeof(int));
int f(P p, Q q, R r) { 1; return p.i + q.i + r.i; }
#pragma clang diagnostic pop
int g() { 2; return 0; }
";
    std::fs::write(d.join("p.cpp"), tu).unwrap();
    assert!(path_note(&d, "p.pch", "p.cpp").contains("declaration image"));
    let o = same(&d, "p.pch", &["-S", "-emit-llvm"], "p.cpp");
    assert_eq!(o.stderr.matches("warning: expression result unused").count(), 1, "{}", o.stderr);
    assert!(o.stderr.contains("p.cpp:9:11"), "{}", o.stderr);
}

/// A header the image cannot carry is replayed whole, with the reason
/// under `-v` on both sides.
#[test]
fn uncarried_header_is_replayed() {
    let d = scratch("fallback");
    let note = produce(&d, "l", "inline auto l = [](int a) { return a; };\n");
    assert!(note.contains("consumers replay the header: "), "{note}");
    std::fs::write(d.join("l.cpp"), "int main() { return l(0); }\n").unwrap();
    let note = path_note(&d, "l.pch", "l.cpp");
    assert!(note.contains("'l.pch' is replayed: it has no declaration image"), "{note}");
    same(&d, "l.pch", &["-S", "-emit-llvm"], "l.cpp");
}

/// Sema state the image does not carry and the consumer depends on (a
/// pending `#pragma weak`, a block-scope `extern` a later definition
/// redeclares) means no image: an image consumer made `f` strong and
/// `v` a second, undefined global.
#[test]
fn uncarried_sema_state_is_replayed() {
    let d = scratch("sema-state");
    for (name, header, tu, why, want) in [
        (
            "w",
            "#pragma weak f\nextern \"C\" int g(int);\n",
            "extern \"C\" int f() { return g(1); }\n",
            "a #pragma weak naming an undeclared identifier",
            "define weak dso_local i32 @f(",
        ),
        (
            "x",
            "inline int g() { extern int v; return v; }\n",
            "int v = 4;\nint h() { return g() + v; }\n",
            "a block-scope extern or function declaration",
            "@v = dso_local global i32 4",
        ),
    ] {
        let note = produce(&d, name, header);
        assert!(note.contains(&format!("consumers replay the header: {why}")), "{note}");
        let tu_file = format!("{name}.cpp");
        std::fs::write(d.join(&tu_file), tu).unwrap();
        let ir = same(&d, &format!("{name}.pch"), &["-S", "-emit-llvm"], &tu_file);
        assert!(ir.stdout.contains(want), "{want}\n{}", ir.stdout);
        assert!(!ir.stdout.contains("@v.1"), "{}", ir.stdout);
    }
}

/// A block-scope `extern` or function declaration the header also
/// declares at namespace scope (libstdc++'s `std::terminate` in
/// `std::__terminate`) is not refused for the sema state: the
/// consumer's redeclaration finds the namespace-scope one. (The image
/// writer refuses the chain itself today, a redeclaration of a
/// block-scope declaration it does not carry; either way the consumer
/// matches replay.)
#[test]
fn redeclared_local_extern_is_not_sema_state() {
    let d = scratch("local-extern");
    let header = "inline int g() { extern int v; return v; }\nextern int v;\n\
                  namespace n { inline void t() { void k(); k(); } }\nnamespace n { void k(); }\n";
    let note = produce(&d, "y", header);
    assert!(!note.contains("a block-scope extern"), "{note}");
    let tu = "int v = 4;\nvoid n::k() {}\nint main() { n::t(); return g() + v; }\n";
    std::fs::write(d.join("y.cpp"), tu).unwrap();
    let ir = same(&d, "y.pch", &["-S", "-emit-llvm"], "y.cpp");
    assert!(ir.stdout.contains("@v = dso_local global i32 4"), "{}", ir.stdout);
    assert!(!ir.stdout.contains("@v.1") && !ir.stdout.contains("@_ZN1n1kEv.1"), "{}", ir.stdout);
    let check = ["-fkiln-pch-image=check", "-include-pch", "y.pch", "-S", "-emit-llvm"];
    let o = ok(&d, &[&check[..], &["y.cpp", "-o", "-"]].concat());
    assert_eq!(o.stderr, "");
}

/// `check` builds the image and replays, and finds them equal.
#[test]
fn check_runs_both() {
    let d = scratch("check");
    produce(&d, "r", RICH);
    std::fs::write(d.join("r.cpp"), RICH_TU).unwrap();
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "r.pch"];
        let o = ok(&d, &[&check[..], mode, &["r.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// A header edited after the PCH was built, its size kept, with the
/// fingerprint check off: the image is not used (its text is not the
/// header's) and the edited header is replayed.
#[test]
fn stale_image_is_replayed() {
    let d = scratch("stale");
    produce(&d, "s", "inline int v() { return 7; }\n");
    std::fs::write(d.join("s.h"), "inline int v() { return 9; }\n").unwrap();
    std::fs::write(d.join("s.cpp"), "int main() { return v(); }\n").unwrap();
    let args = ["-v", "-Xclang", "-fno-validate-pch", "-fkiln-pch-image=load", "-include-pch"];
    let o = ok(&d, &[&args[..], &["s.pch", "-S", "-emit-llvm", "-o", "-", "s.cpp"]].concat());
    assert!(o.stderr.contains("'s.pch' is replayed: 's.h' changed"), "{}", o.stderr);
    assert!(o.stdout.contains("ret i32 9"), "{}", o.stdout);
}

/// A PCH whose image sections are damaged is an error, never a guess.
#[test]
fn damaged_image_is_an_error() {
    let d = scratch("damaged");
    produce(&d, "x", "int x();\n");
    let mut b = std::fs::read(d.join("x.pch")).unwrap();
    let at = b.len() - 20;
    b[at] ^= 0xff;
    std::fs::write(d.join("x.pch"), b).unwrap();
    std::fs::write(d.join("x.cpp"), "int y = x();\n").unwrap();
    let o = kiln(&d, &["-fkiln-pch-image=load", "-include-pch", "x.pch", "-fsyntax-only", "x.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("malformed or corrupted precompiled file"), "{}", o.stderr);
}
