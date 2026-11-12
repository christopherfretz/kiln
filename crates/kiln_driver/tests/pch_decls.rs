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

/// spec/modules.md §11.11 (WP-1284; [dcl.enum]/2, [class.pre]/1): an
/// unnamed enumeration or class no typedef names (glibc's `<fenv.h>`
/// declares its `FE_*` constants so) is carried; it was an internal
/// image inconsistency.
#[test]
fn unnamed_types_are_carried() {
    let d = scratch("unnamed");
    let header = "\
enum { FE_A = 1, FE_B = 2 };
struct { int z; } anon_v = {3};
inline int anon_sum() { return FE_A + FE_B + anon_v.z; }
";
    let note = produce(&d, "u", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    std::fs::write(d.join("u.cpp"), "int main() { return anon_sum() + FE_B + anon_v.z; }\n")
        .unwrap();
    same(&d, "u.pch", &["-S", "-emit-llvm", "-O0"], "u.cpp");
}

/// spec/modules.md §11.11 "Expressions in types" (WP-1300): types and
/// template arguments holding an expression are carried, and a consumer
/// substitutes them as replay does ([temp.alias]/2, [temp.arg.nontype]):
/// `decltype(nullptr)` ([lex.nullptr]), non-type template arguments
/// written as expressions, dependent and non-dependent
/// `decltype(e)` ([dcl.type.decltype]), `noexcept(e)` on a function type
/// ([except.spec]/2; an operand naming the function's parameters is set
/// after the bodies), value-dependent array bounds ([temp.dep.constexpr])
/// and a pack expansion of a known length ([temp.variadic]).
#[test]
fn expressions_in_types_are_carried() {
    let d = scratch("exprs");
    let header = "\
typedef decltype(nullptr) nullptr_t;
inline nullptr_t np() { return nullptr; }
template <class T, T v> struct ic { static constexpr T value = v; using type = ic; };
template <bool B> using bc = ic<bool, B>;
template <class T> struct is_ptr : ic<bool, false> {};
template <class T> struct is_ptr<T *> : ic<bool, true> {};
template <class T> using size_of = ic<unsigned long, sizeof(T)>;
template <class T> using not_ptr = bc<!is_ptr<T>::value>;
using two = ic<int, 1 + 1>;
inline int g = 4;
inline decltype(g + 1L) dl() { return g + 1L; }
template <class T> decltype(T() + 1) plus1(T t) { return t + 1; }
template <class T> void sw(T &) noexcept(noexcept(T())) {}
template <class T> struct pr { T a; void swap(pr &o) noexcept(noexcept(a = o.a)) { a = o.a; } };
template <class T> void swap(pr<T> &x, pr<T> &y) noexcept(noexcept(x.swap(y))) { x.swap(y); }
inline void h(int &x) noexcept(noexcept(x + 1)) {}
inline void nx() noexcept(sizeof(int) > 2) {}
template <class T> struct arr { T data[sizeof(T) + 1]; };
template <int N> struct buf { char b[N * 2]; };
template <class... Ts> struct tup { static constexpr unsigned long n = sizeof...(Ts); };
template <class... Ts> using tup2 = tup<Ts..., Ts...>;
";
    let note = produce(&d, "x", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "\
int main() {
  nullptr_t z = np(); (void)z;
  static_assert(bc<true>::value && !is_ptr<int>::value && is_ptr<int *>::value);
  static_assert(size_of<int>::value == sizeof(int) && not_ptr<int>::value && two::value == 2);
  static_assert(!not_ptr<int *>::value && noexcept(nx()));
  int i = 1; sw(i); static_assert(noexcept(sw(i)));
  pr<int> p{1}, q{2}; swap(p, q); h(i); static_assert(noexcept(swap(p, q)) && noexcept(h(i)));
  arr<short> a{}; buf<3> b{};
  static_assert(sizeof(a.data) == 3 * sizeof(short) && sizeof(b.b) == 6 && tup2<int, char>::n == 4);
  return int(dl()) + plus1(1) + int(sizeof a) + int(sizeof b);
}
";
    std::fs::write(d.join("x.cpp"), tu).unwrap();
    assert!(path_note(&d, "x.pch", "x.cpp").contains("': declaration image ("));
    same(&d, "x.pch", &["-fsyntax-only", "-Wall"], "x.cpp");
    same(&d, "x.pch", &["-S", "-emit-llvm", "-O0"], "x.cpp");
    let check = ["-fkiln-pch-image=check", "-include-pch", "x.pch", "-S", "-emit-llvm"];
    assert_eq!(ok(&d, &[&check[..], &["x.cpp", "-o", "-"]].concat()).stderr, "");
    // A substitution failure inside a carried expression is diagnosed as
    // replay diagnoses it.
    std::fs::write(
        d.join("y.cpp"),
        "struct S {};\nint k = plus1(S());\nbuf<-1> nb;\nint v = sizeof(arr<void>);\n\
          struct N { N &operator=(const N &) = delete; };\nvoid u() { pr<N> a, b; swap(a, b); }\n",
    )
    .unwrap();
    let o = same(&d, "x.pch", &["-fsyntax-only"], "y.cpp");
    assert!(o.stderr.contains("substitution failure [with T = S]"), "{}", o.stderr);
    assert!(o.stderr.contains("in instantiation of exception specification"), "{}", o.stderr);
    assert!(o.stderr.contains("5 errors generated"), "{}", o.stderr);
}

/// Sema state the image does not carry and the consumer depends on (a
/// pending `#pragma weak`) means no image: an image consumer made `f`
/// strong.
#[test]
fn uncarried_sema_state_is_replayed() {
    let d = scratch("sema-state");
    let note = produce(&d, "w", "#pragma weak f\nextern \"C\" int g(int);\n");
    let why = "a #pragma weak naming an undeclared identifier";
    assert!(note.contains(&format!("consumers replay the header: {why}")), "{note}");
    std::fs::write(d.join("w.cpp"), "extern \"C\" int f() { return g(1); }\n").unwrap();
    let ir = same(&d, "w.pch", &["-S", "-emit-llvm"], "w.cpp");
    assert!(ir.stdout.contains("define weak dso_local i32 @f("), "{}", ir.stdout);
}

/// WP-1314: block-scope static assertions ([dcl.pre]/10, checked again by
/// each instantiation, [temp.inst]/4), block-scope typedef and alias
/// declarations ([dcl.typedef]) and names with explicit template
/// arguments ([temp.arg.explicit]/1) are carried; a failing assertion is
/// diagnosed as replay diagnoses it.
#[test]
fn local_static_asserts_and_explicit_args_take_the_image() {
    let d = scratch("wp1314-lsa");
    let header = "\
template <class T> int f(T t) { static_assert(sizeof(T) < 64, \"small\"); return int(t) + 1; }
inline int g(int x) { static_assert(sizeof(int) >= 2); return x * 2; }
template <class T> struct S { static int h() { static_assert(sizeof(T) != 3); return 7; } };
template <class T> T pick(T a) { return a; }
inline int callpick() { return pick<int>(3) + pick<long>(4L); }
template <class T> long td(T t) { typedef const T *P; using L = long; P p = &t; return L(*p); }
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    std::fs::write(
        d.join("a.cpp"),
        "int main() { return f(3) + g(2) + S<int>::h() + callpick() + int(td(1)); }\n",
    )
    .unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
    std::fs::write(
        d.join("b.cpp"),
        "struct Big { char c[100]; operator int() const; };\nstruct T3 { char c[3]; };\n\
         int a = f(Big{});\nint b = S<T3>::h();\n",
    )
    .unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "b.cpp");
    assert!(o.stderr.contains("static assertion failed due to requirement"), "{}", o.stderr);
    assert!(o.stderr.contains("'sizeof(T3) != 3'"), "{}", o.stderr);
}

/// A block-scope `extern` or function declaration in the header
/// ([basic.link]/6) is carried with its link (WP-1293): the unit's
/// namespace-scope declarations redeclare it as replay's do, in another
/// block of its namespace (an inline one too), and the header's own
/// later namespace-scope declaration stays one entity with it. Before, an image consumer made `v` a second, undefined
/// global.
#[test]
fn local_externs_take_the_image() {
    let d = scratch("local-extern");
    let header = "inline int g() { extern int v; return v; }\n\
                  namespace n { inline void t() { void k(); k(); } }\nnamespace n { void k(); }\n\
                  namespace n { inline namespace i { inline long u() { extern long w; return w; } } }\n\
                  inline int h() { int f(int = 2); return f(); }\n";
    let note = produce(&d, "y", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "int v = 4;\nvoid n::k() {}\nnamespace n { inline namespace i { long w = 5; } }\nint f(int a) { return a; }\n\
              int main() { n::t(); return g() + v + int(n::u()) + h(); }\n";
    std::fs::write(d.join("y.cpp"), tu).unwrap();
    let ir = same(&d, "y.pch", &["-S", "-emit-llvm"], "y.cpp");
    for want in ["@v = dso_local global i32 4", "@_ZN1n1i1wE = dso_local global i64 5"] {
        assert!(ir.stdout.contains(want), "{want}\n{}", ir.stdout);
    }
    for bad in ["@v.1", "@_ZN1n1kEv.1", "@_ZN1n1i1wE.1", "@_Z1fi.1"] {
        assert!(!ir.stdout.contains(bad), "{bad}\n{}", ir.stdout);
    }
    let check = ["-fkiln-pch-image=check", "-include-pch", "y.pch", "-S", "-emit-llvm"];
    let o = ok(&d, &[&check[..], &["y.cpp", "-o", "-"]].concat());
    assert_eq!(o.stderr, "");
    // A conflicting redeclaration is diagnosed as replay diagnoses it.
    let bad = "static int v;\nnamespace n { int k(); }\nlong f(int);\n";
    std::fs::write(d.join("z.cpp"), bad).unwrap();
    let o = same(&d, "y.pch", &["-fsyntax-only"], "z.cpp");
    assert_ne!(o.code, 0);
    for want in [
        "static declaration of 'v' follows non-static declaration",
        "functions that differ only in their return type cannot be overloaded",
    ] {
        assert!(o.stderr.contains(want), "{want}\n{}", o.stderr);
    }
}

/// Attributes on block-scope declarations ([dcl.attr.grammar]/5; the
/// shape of libstdc++'s `std::__terminate`, a GNU-attributed block-scope
/// redeclaration) and the mem-initializers of constructors of class
/// templates as parsed ([class.base.init], [temp.inst]/4: members, a
/// dependent base through a typedef, braced lists, delegation, a pack
/// expansion, a constructor template, an out-of-line definition) are
/// carried (WP-1310), and a referenced pattern leaves its template
/// unreferenced as replay does.
#[test]
fn local_attributes_and_pattern_inits_are_carried() {
    let d = scratch("wp1310");
    let header = "\
namespace s {
void stop(int) noexcept;
inline void call_stop(int c) noexcept {
  void stop(int) noexcept __attribute__((__noreturn__, __cold__));
  if (c) stop(c);
}
}
inline int locals() { [[maybe_unused]] int a = 3; __attribute__((unused)) int b = 4; return a + b; }
template <class T> struct Base { T b; Base(T x) : b(x) {} };
template <class T> struct Iter {
  T *p;
  Iter() : p(nullptr) {}
  explicit Iter(T *q) : p(q) {}
  template <class U> Iter(const Iter<U> &o) : p(o.p) {}
};
template <class T> struct Der : Base<T> {
  typedef Base<T> base_type;
  int k, arr[2];
  Der(T x) : base_type(x), k{3}, arr{1, 2} {}
  Der() : Der(T(7)) {}
};
template <class... Bs> struct Multi : Bs... { Multi(int v) : Bs(v)... {} };
struct B1 { int a; B1(int x) : a(x) {} };
struct B2 { int c; B2(int x) : c(x + 1) {} };
template <class T> struct Out { T v; Out(T); };
template <class T> Out<T>::Out(T x) : v(x * 2) {}
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "\
void s::stop(int) noexcept { __builtin_abort(); }
int main() {
  int x = 4; s::call_stop(0);
  Iter<int> a(&x); Iter<const int> b(a); Iter<int> z;
  Der<int> d(5); Der<long> e; Multi<B1, B2> m(10); Out<int> o(3);
  return *b.p + !z.p + d.b + d.k + d.arr[1] + int(e.b) + m.a + m.c + o.v + locals();
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    let ir = same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    assert!(ir.stdout.contains("cold noreturn"), "{}", ir.stdout);
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
    // A mem-initializer that does not instantiate is diagnosed as replay
    // diagnoses it.
    std::fs::write(
        d.join("b.cpp"),
        "struct NC {};\nvoid f() { Out<NC> o(NC{}); Der<NC> d; Multi<B1> m; }\n",
    )
    .unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "b.cpp");
    assert!(o.stderr.contains("invalid operands to binary expression"), "{}", o.stderr);
    assert!(o.stderr.contains("no matching constructor"), "{}", o.stderr);
}

/// Redeclarations of the replaceable global allocation functions (the
/// shape of libstdc++'s `<new>`, [new.delete]/2: they redeclare the
/// implicit ones) and a dependent direct-initialization in a class
/// template (`<bits/postypes.h>`, [dcl.init.general]/16) are carried
/// (WP-1320).
#[test]
fn global_new_delete_and_paren_inits_are_carried() {
    let d = scratch("wp1320a");
    let header = "\
namespace std { using size_t = decltype(sizeof 0); enum class align_val_t : size_t {}; }
void *operator new(std::size_t);
void operator delete(void *) noexcept;
void *operator new(std::size_t, std::align_val_t);
inline void *operator new(std::size_t, void *p) noexcept { return p; }
template <class T> struct Pos {
  T off;
  Pos(T o) : off(o) {}
  Pos &operator+=(T x) { off += x; return *this; }
  Pos operator+(T x) const { Pos p(*this); p += x; return p; }
};
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "\
int main() {
  int *p = new int(3); int r = *p; delete p;
  alignas(int) unsigned char b[sizeof(int)]; r += *::new (b) int(4);
  Pos<long> a(1); r += int((a + 2).off);
  return r - 10;
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    let ir = same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    assert!(ir.stdout.contains("@_Znwm"), "{}", ir.stdout);
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
    // The unit's replacement of a header-declared global allocation
    // function is a definition of the same function.
    std::fs::write(
        d.join("b.cpp"),
        "void *operator new(std::size_t n) { return __builtin_malloc(n); }\nint *f() { return new int; }\n",
    )
    .unwrap();
    let o = same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "b.cpp");
    assert!(o.stdout.contains("define dso_local noundef nonnull ptr @_Znwm"), "{}", o.stdout);
}

/// WP-1320: explicit specializations that take a non-type default
/// argument (the shape of `__gnu_cxx::__promote`) are carried, the
/// default as an expression; a default that instantiates a class with a
/// static data member keeps the header replayed.
#[test]
fn nttp_default_explicit_specs_are_carried() {
    let d = scratch("wp1320b");
    let header = "\
template <class T> struct IsInt { enum { value = 0 }; };
template <> struct IsInt<int> { enum { value = 1 }; };
template <class T, bool = IsInt<T>::value> struct Promote { typedef double type; };
template <class T> struct Promote<T, false> {};
template <> struct Promote<float> { typedef float type; };
template <> struct Promote<long double> { typedef long double type; };
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "\
int main() {
  Promote<int>::type x = 1.5; Promote<float>::type y = 2.5f;
  return int(x + y) - 4 + int(sizeof(Promote<long double>::type) != sizeof(long double));
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
    let refused = "\
template <class T> struct IsI { static constexpr bool value = false; };
template <class T, bool = IsI<T>::value> struct X { int a; };
template <> struct X<long> { long b; };
";
    let note = produce(&d, "b", refused);
    assert!(
        note.contains("replay the header: an explicit specialization using a non-type default"),
        "{note}"
    );
}

/// WP-1320: a dependent `X<T>` completed with its defaults (its
/// canonical type is `X<T, d>`) and a base that is a dependent member
/// type (`typename X<T>::type`) are carried.
#[test]
fn dependent_default_specializations_are_carried() {
    let d = scratch("wp1320c");
    let header = "\
template <bool B> struct BC { static constexpr bool value = B; typedef BC type; };
template <class T> struct IsA : BC<false> {};
template <> struct IsA<int> : BC<true> {};
template <class T, bool = IsA<T>::value> struct H : BC<false> {};
template <class T> struct H<T, true> : BC<true> {};
template <class T> struct Is : H<T>::type {};
template <class T, int N = sizeof(T)> struct Sz { typedef char type[N]; };
template <class T> struct Hold { typename Sz<T>::type b; };
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "\
struct S {};
int main() {
  Hold<long> h;
  return (Is<int>::value ? 0 : 1) + (Is<S>::value ? 2 : 0) + (sizeof(h.b) == sizeof(long) ? 0 : 4);
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// WP-1342: `header` takes the image, and `tu` compiled against it
/// agrees with replay (`-fkiln-pch-image=check`) for syntax and IR, and
/// runs to exit code 0.
fn image_agrees(name: &str, header: &str, tu: &str) {
    let d = scratch(name);
    std::fs::write(d.join("a.h"), header).unwrap();
    let o =
        ok(&d, &["-std=c++20", "-v", "-fkiln-pch-image", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("consumers use its declaration image"), "{}", o.stderr);
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    let load = ["-std=c++20", "-fkiln-pch-image=load", "-include-pch", "a.pch"];
    let o = ok(&d, &[&load[..], &["-v", "-fsyntax-only", "a.cpp"]].concat());
    assert!(o.stderr.contains("': declaration image ("), "{}", o.stderr);
    same(&d, "a.pch", &["-std=c++20", "-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-std=c++20", "-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
    ok(&d, &[&load[..], &["a.cpp", "-o", "a.out"]].concat());
    let run = Command::new(d.join("a.out")).status().unwrap();
    assert_eq!(run.code(), Some(0));
}

/// WP-1342: template template parameters (defaults, packs, in partial
/// specializations of class and variable templates, named with
/// arguments) and dependent template names (`M<T>::template apply`) as
/// template arguments.
#[test]
fn template_template_parameters_are_carried() {
    let header = "\
template <class T> struct box { T v; };
template <class T, class U = int> struct duo { T a; U b; };
template <template <class...> class F, class... A> struct lazy : F<A...> {};
template <template <class...> class T, class... A, class = T<A...>>
constexpr bool valid(int) { return true; }
template <template <class...> class, class...> constexpr bool valid(...) { return false; }
template <class T> struct first;
template <template <class, class...> class S, class T, class... A>
struct first<S<T, A...>> { using type = T; };
template <class T, template <class...> class P> constexpr bool is_spec = false;
template <template <class...> class P, class... A> constexpr bool is_spec<P<A...>, P> = true;
template <class T, template <class> class W = box> struct wrap { W<T> w; };
template <template <class> class... Ws> struct many { static constexpr int n = sizeof...(Ws); };
template <template <class> class W, class T> constexpr int wsize() { return sizeof(W<T>); }
template <class T> struct meta { template <class U> struct apply { duo<T, U> d; }; };
template <class T, template <class> class Q> struct with { Q<T> q; };
template <class T> using meta_of = with<long, meta<T>::template apply>;
template <class T> struct cell { T v; cell(T t) : v(t) {} };
template <template <class...> class C, class A> auto make(A a) { return C(a); }
";
    let tu = "\
int main() {
  static_assert(is_spec<duo<int>, duo> && !is_spec<box<int>, duo>);
  static_assert(valid<box, int>(0) && !valid<box, int, int>(0));
  static_assert(sizeof(first<duo<char, long>>::type) == 1 && many<box, box>::n == 2);
  wrap<int> w{{3}};
  lazy<duo, char, short> l{};
  meta_of<char> m{};
  return w.w.v - 3 + int(sizeof(l) - 4) + wsize<box, char>() - 1
       + int(sizeof(m.q.d) - sizeof(duo<char, long>)) + make<cell>(2).v - 2;
}
";
    image_agrees("wp1342a", header, tu);
}

/// WP-1342: partial specializations with requires-clauses and
/// type-constraints, ordered by subsumption.
#[test]
fn constrained_partial_specializations_are_carried() {
    let header = "\
template <class T> constexpr bool is_ref_v = false;
template <class T> constexpr bool is_ref_v<T&> = true;
template <class T> concept ref = is_ref_v<T>;
template <class T> concept small = sizeof(T) <= 4;
template <class T> concept has_type = requires { typename T::type; };
template <class A, class B> struct pick { static constexpr int value = 0; };
template <class A, class B> requires ref<A> && has_type<B>
struct pick<A, B> { static constexpr int value = 1; };
template <class A, class B> requires ref<A> && has_type<B> && small<B>
struct pick<A, B> { static constexpr int value = 2; };
template <small A, class B> struct pick<A*, B> { static constexpr int value = 3; };
template <class T> constexpr int kind = 0;
template <class T> requires small<T> constexpr int kind<T> = 1;
";
    let tu = "\
struct HT { using type = int; };
struct Big { using type = int; long a, b; };
int main() {
  static_assert(pick<int, HT>::value == 0 && pick<int&, HT>::value == 2);
  static_assert(pick<int&, Big>::value == 1 && pick<int*, Big>::value == 3);
  static_assert(kind<char> == 1 && kind<Big> == 0);
  return pick<Big*, Big>::value;
}
";
    image_agrees("wp1342b", header, tu);
}

/// WP-1342: partial specializations of member templates, of a class
/// and of a class template.
#[test]
fn member_partial_specializations_are_carried() {
    let header = "\
struct base {
protected:
  template <class...> struct list {};
  template <class T, class... U> struct list<T, U...> : list<U...> {
    static constexpr int size = sizeof(T);
  };
  template <class T> static constexpr int vt = 3;
  template <class T> static constexpr int vt<T&> = 4;
public:
  static int f();
};
template <class T> struct outer {
  template <class U, class = void> struct ptr { static constexpr int v = 0; };
  template <class U> struct ptr<U*, void> { static constexpr int v = sizeof(T); };
  template <class U> requires (sizeof(U) == 8) struct ptr<U, void> { static constexpr int v = 8; };
  template <class U, bool = true> union st { U u; };
  template <class U> union st<U, false> { U u; T t; };
};
";
    let tu = "\
int base::f() { return list<char, int>::size + vt<int&>; }
int main() {
  static_assert(outer<char>::ptr<int>::v == 0 && outer<short>::ptr<int*>::v == 2);
  static_assert(outer<short>::ptr<long>::v == 8);
  static_assert(sizeof(outer<long>::st<char, false>) == sizeof(long));
  return base::f() - 5;
}
";
    image_agrees("wp1342c", header, tu);
}

/// WP-1342: calls in templates through using-declarations, and a
/// qualified class template name deduced from an initializer.
#[test]
fn using_shadow_calls_and_qualified_deduction_are_carried() {
    let header = "\
namespace lib {
inline int mag(int x) { return x < 0 ? -x : x; }
inline long mag(long x) { return x < 0 ? -x : x; }
template <class T> T twice(T t) { return t + t; }
template <class T> struct cell { T v; cell(T t) : v(t) {} };
}
namespace det {
using lib::mag;
using lib::twice;
template <class T> T alt(T t) { return mag(t) + twice(t); }
template <class T> struct holder { static T go(T t) { return twice<T>(mag(t)); } };
template <class T> T wrapped(T t) { return lib::cell(t).v; }
}
";
    let tu = "\
int main() { return det::alt(-2) + det::holder<long>::go(-3) + det::wrapped(1) == 5 ? 0 : 1; }
";
    image_agrees("wp1342d", header, tu);
}

/// [`image_agrees`], and the census of `header` refuses nothing.
fn image_carries(name: &str, header: &str, tu: &str) {
    let d = scratch(&format!("{name}-census"));
    std::fs::write(d.join("a.h"), header).unwrap();
    let args = ["-std=c++20", "-v", "-fkiln-pch-image", "-fkiln-bmi-image-census=all"];
    let o = ok(&d, &[&args[..], &["-x", "c++-header", "a.h", "-o", "a.pch"]].concat());
    assert!(!o.stderr.contains("refused: "), "{}", o.stderr);
    image_agrees(name, header, tu);
}

/// WP-1344: a pack in a specialization's argument list (`_List<unsigned
/// char, ...>` in `<type_traits>`).
#[test]
fn pack_arguments_are_carried() {
    let header = "\
template <class...> struct L {};
template <class T, class... U> struct L<T, U...> : L<U...> { static constexpr int n = sizeof(T); };
template <int N, class T, bool = (N <= T::n)> struct sel;
template <int N, class U, class... Us> struct sel<N, L<U, Us...>, true> { using type = U; };
template <int N, class U, class... Us> struct sel<N, L<U, Us...>, false> : sel<N, L<Us...>> {};
template <class T> struct pick {
  using Ls = L<unsigned char, unsigned short, unsigned int, unsigned long>;
  using type = typename sel<sizeof(T), Ls>::type;
};
";
    let tu = "\
int main() { pick<int>::type x = 3; return sizeof(x) == 4 ? 0 : 1; }
";
    image_carries("wp1344a", header, tu);
}

/// WP-1344: a member typedef of a completed specialization of a partial
/// specialization named through a base (`__make_unsigned_selector`'s shape).
#[test]
fn member_typedefs_of_partial_specializations_are_carried() {
    let header = "\
typedef unsigned long sz;
template <class Q, class U> struct cv { using type = U; };
class base {
protected:
  template <class...> struct L {};
  template <class T, class... U> struct L<T, U...> : L<U...> { static constexpr sz n = sizeof(T); };
  template <sz N, class T, bool = (N <= T::n)> struct sel;
  template <sz N, class U, class... Us> struct sel<N, L<U, Us...>, true> { using type = U; };
  template <sz N, class U, class... Us> struct sel<N, L<U, Us...>, false> : sel<N, L<Us...>> {};
};
template <class T, bool I = false, bool E = true> class pick;
template <class T> class pick<T, false, true> : base {
  using Ls = L<unsigned char, unsigned short, unsigned int, unsigned long>;
  using ut = typename sel<sizeof(T), Ls>::type;
public:
  using type = typename cv<T, ut>::type;
};
template <class T> struct mk { using type = T; };
template <> struct mk<wchar_t> { using type = typename pick<wchar_t, false, true>::type; };
";
    let tu = "\
int main() { mk<wchar_t>::type x = 3; return sizeof(x) == sizeof(wchar_t) ? 0 : 1; }
";
    image_carries("wp1344b", header, tu);
}

/// WP-1344: specializations of constrained templates whose constraints
/// are the template's own (a requires-clause, a type-constraint), left
/// incomplete or completed.
#[test]
fn specializations_of_constrained_templates_are_carried() {
    let header = "\
template <class T> concept small = sizeof(T) <= 4;
template <class T> requires small<T> struct box { T v; int get() const { return int(v); } };
template <small T> struct cell { T v; };
template <class T> struct opt { T v; int f() const requires small<T> { return 1; } int f() const { return 2; } };
template <class T> struct pp { static constexpr int k = 0; };
template <class T> requires (sizeof(T) == 2) struct pp<T*> { static constexpr int k = 2; };
using bi = box<int>;
inline int use(bi b) { return b.get(); }
inline cell<short> cs{4};
using oi = opt<char>;
using ppi = pp<short*>;
template <class T> T twice(T t) requires small<T> { return t + t; }
inline int tw() { return twice(3); }
";
    let tu = "\
int main() {
  bi b{5}; oi o{}; static_assert(pp<short*>::k == 2 && pp<long*>::k == 0);
  return use(b) - 5 + cs.v - 4 + o.f() - 1 + tw() - 6 + box<char>{1}.get() - 1;
}
";
    image_carries("wp1344c", header, tu);
}

/// WP-1344: explicit and partial specializations whose arguments are
/// expressions (`get<0>` with a `size_t` parameter) or enumerators (scoped
/// and unscoped).
#[test]
fn explicit_specialization_value_arguments_are_carried() {
    let header = "\
typedef unsigned long sz;
template <sz I> struct get;
template <> struct get<0> { static int f() { return 10; } };
template <> struct get<1> { static int f() { return 20; } };
template <long A, long B> struct gcd { static const long value = 7; };
template <> struct gcd<0, 0> { static const long value = 1; };
enum pol { single, multi };
template <pol P> struct mtx { static constexpr int k = 1; };
template <> struct mtx<multi> { static constexpr int k = 2; };
enum class fmt { seq, map };
template <fmt F, class T> struct rf { static constexpr int v = 0; };
template <class T> struct rf<fmt::map, T> { static constexpr int v = sizeof(T); };
inline int k1 = mtx<multi>::k + rf<fmt::map, short>::v;
";
    let tu = "\
int main() {
  static_assert(gcd<0, 0>::value == 1 && gcd<1, 2>::value == 7 && rf<fmt::seq, int>::v == 0);
  return get<0>::f() + get<1>::f() - 30 + mtx<single>::k + k1 - 5;
}
";
    image_carries("wp1344d", header, tu);
}

/// WP-1344: a class template pattern whose base names its own template
/// (`__common_reference_impl` in `<type_traits>`).
#[test]
fn self_referential_pattern_bases_are_carried() {
    let header = "\
template <class A, class B, int N = 1, class = void>
struct impl : impl<A, B, N + 1> {};
template <class A, class B> struct impl<A, B, 3, void> { using type = int; };
template <class A, class B> struct impl<A, B, 2, decltype(void(sizeof(A) == 4))> { using type = A; };
template <class T> struct chain : chain<T*> {};
template <class T> struct chain<T***> { static constexpr int depth = 3; };
";
    let tu = "\
int main() { impl<int, char>::type x = 2; return x + chain<int>::depth - 5; }
";
    image_carries("wp1344e", header, tu);
}

/// WP-1344: a static data member's in-class initializer the importer
/// needs while it builds the records (a default template argument of a
/// member of a completed specialization).
#[test]
fn static_member_initializers_read_during_completion() {
    let header = "\
template <class T> struct A { static constexpr int n = sizeof(T); };
template <class T, bool = (A<T>::n > 2)> struct B { static constexpr int v = 1; };
template <class T> struct B<T, false> { static constexpr int v = 2; };
template <class T> struct C { using t = B<T>; int x; };
inline int c = sizeof(C<int>);
inline int d = sizeof(C<char>);
";
    let tu = "\
int main() { return C<int>::t::v + C<char>::t::v == 3 ? 0 : 1; }
";
    image_carries("wp1344f", header, tu);
}

/// WP-1344: a non-type parameter pack's expansion in a partial
/// specialization's arguments (`__static_max<_I0, _I1, _In...>` in
/// `<__type_traits/aligned_union.h>`), and a pattern's static member
/// initializer naming a specialization with expression arguments.
#[test]
fn partial_specialization_pack_arguments_are_carried() {
    let header = "\
typedef unsigned long sz;
template <sz... I> struct smax;
template <sz I0> struct smax<I0> { static const sz value = I0; };
template <sz I0, sz I1, sz... In> struct smax<I0, I1, In...> {
  static const sz value = I0 >= I1 ? smax<I0, In...>::value : smax<I1, In...>::value;
};
template <sz L, class T0, class... Ts> struct au {
  static const sz len = smax<L, sizeof(T0), sizeof(Ts)...>::value;
};
template <int... N> struct seq {};
template <class S, int K> struct add;
template <int... N, int K> struct add<seq<N...>, K> { using type = seq<N..., K>; };
";
    let tu = "\
int main() { add<seq<1, 2>, 3>::type s; (void)s; return au<2, char, int, short>::len == 4 ? 0 : 1; }
";
    image_carries("wp1344g", header, tu);
}

/// WP-1344: a member function template whose exception specification
/// names a function template specialization (`std::ranges::swap`'s
/// `noexcept(noexcept(std::declval<const _Swap&>()(...)))`).
#[test]
fn exception_specifications_naming_specializations_are_carried() {
    let header = "\
template <class T> T &&dv() noexcept;
struct sw {
  template <class T> constexpr void operator()(T &a, T &b) const noexcept(noexcept(dv<const sw &>()(a, b))) { T t = a; a = b; b = t; }
  template <class T, unsigned long N>
  constexpr void operator()(T (&a)[N], T (&b)[N]) const noexcept(noexcept(dv<const sw &>()(*a, *b))) {
    for (unsigned long n = 0; n < N; ++n) (*this)(a[n], b[n]);
  }
};
inline constexpr sw swp{};
";
    let tu = "\
int main() { int a[2] = {1, 2}, b[2] = {3, 4}; swp(a, b); int x = 1, y = 2; swp(x, y); return a[0] == 3 && x == 2 ? 0 : 1; }
";
    image_carries("wp1344h", header, tu);
}

/// WP-1344: dependent specializations of `__make_integer_seq` and
/// `__type_pack_element` (`<__utility/integer_sequence.h>`,
/// `<bits/utility.h>`).
#[test]
fn builtin_template_specializations_are_carried() {
    let header = "\
typedef unsigned long sz;
template <class T, T... V> struct iseq { static constexpr sz n = sizeof...(V); };
template <sz... I> struct idx {};
template <class T, T N> using make_seq = __make_integer_seq<iseq, T, N>;
template <sz N> struct mk {
  template <class, sz... I> using tup = idx<I...>;
  using type = __make_integer_seq<tup, sz, N>;
};
template <sz I, class... T> struct nth { using type = __type_pack_element<I, T...>; };
";
    let tu = "\
int main() { mk<3>::type t; (void)t; nth<1, char, int>::type x = 2; return make_seq<int, 4>::n == 4 && x == 2 ? 0 : 1; }
";
    image_carries("wp1344i", header, tu);
}

/// Block-scope `static_assert` declarations, typedefs and alias
/// declarations (`declval`'s and `__call_is_nt`'s shapes in `<type_traits>`)
/// are carried (WP-1320).
#[test]
fn local_asserts_and_aliases_are_carried() {
    let d = scratch("wp1320l");
    let header = "\
template <class T> struct Id { typedef T type; };
template <class T> struct No { static constexpr bool value = false; };
template <class T> T &&dv() noexcept { static_assert(No<T>::value, \"odr\"); return dv<T>(); }
template <class T> constexpr int sz() { using U = typename Id<T>::type; typedef U V; static_assert(sizeof(V) > 0); return sizeof(U) + sizeof(V); }
inline int plain() { using I = int; static_assert(sizeof(I) == 4, \"int\"); I x = 3; return x; }
template <class T> constexpr bool nt = noexcept(dv<T>());
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "\
static_assert(sz<int>() == 8 && nt<int>);
int main() { return sz<char>() + plain() == 5 ? 0 : 1; }
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// A dependent array parameter's adjusted type names the canonical
/// template parameter type (WP-1320, type tag 24).
#[test]
fn dependent_array_parameters_are_carried() {
    let d = scratch("wp1320arr");
    let header = "\
template <class T> T first2(const T a[2]) { return a[0] + a[1]; }
template <class T> struct H { template <class U> T at(const U k[3]) const { return T(k[2]); } };
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "int main() { int a[2] = {1, 2}; short k[3] = {0, 0, 3}; return first2(a) + H<int>{}.at(k) == 6 ? 0 : 1; }\n";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// GNU complex types (`__complex__ float` in `<complex>`) are carried
/// (WP-1320, type tag 23).
#[test]
fn complex_types_are_carried() {
    let d = scratch("wp1320x");
    let header = "\
typedef __complex__ float cf;
inline cf mk(float a, float b) { cf z; __real__ z = a; __imag__ z = b; return z; }
template <class T> struct Box { _Complex double v; T re() const { return T(__real__ v); } };
inline _Complex double twice(_Complex double z) { return z * 2.0; }
";
    let note = produce(&d, "a", header);
    assert!(note.contains("consumers use its declaration image"), "{note}");
    let tu = "int main() { Box<int> b{twice(3.0)}; return b.re() == 6 && __imag__ mk(1, 2) == 2 ? 0 : 1; }\n";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': declaration image ("));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// Reduced from `module.global.frag/gmf-floatn-limits.cpp` under
/// `--pch-diff check` (WP-1320 deliverable 5, not fixed): an importer's
/// header declarations loaded from the image record no cross-unit
/// deduplication keys (spec/modules.md §6), so an imported module's GMF
/// that includes the same header re-declares `size_t` and redefines `lim`
/// (an invalid template) instead of being skipped by position, and the
/// declarations differ from the replay's. The fix carries the keys of the
/// header's namespace-scope declarations in the image (or records them
/// while reading it).
#[test]
#[ignore = "WP-1320 follow-up: image declarations record no §6 dedup keys for an imported GMF"]
fn gmf_reinclusion_merges_with_image_declarations() {
    let d = scratch("wp1320g");
    let header = "\
#ifndef A_H
#define A_H
typedef unsigned long size_t;
template <class T> struct lim { static constexpr int d = 1; };
template <> struct lim<float> { static constexpr size_t d = 24; };
#endif
";
    std::fs::write(d.join("a.h"), header).unwrap();
    let o =
        ok(&d, &["-std=c++20", "-v", "-fkiln-pch-image", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("consumers use its declaration image"), "{}", o.stderr);
    let m = "module;\n#include \"a.h\"\nexport module m;\nexport constexpr int get() { return lim<float>::d == 24; }\n";
    std::fs::write(d.join("m.cppm"), m).unwrap();
    ok(&d, &["-std=c++20", "--precompile", "-x", "c++-module", "m.cppm", "-o", "m.pcm"]);
    std::fs::write(d.join("t.cpp"), "import m;\nstatic_assert(get() == 1);\nint main() {}\n")
        .unwrap();
    let check =
        ["-std=c++20", "-fmodule-file=m=m.pcm", "-fkiln-pch-image=check", "-include-pch", "a.pch"];
    let o = kiln(&d, &[&check[..], &["-fsyntax-only", "t.cpp"]].concat());
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

/// A header template's definition-time warning is repeated by the replay
/// and not by the image's consumer; `check` accepts that (WP-1320).
#[test]
fn check_accepts_header_warnings() {
    let d = scratch("wp1320w");
    std::fs::write(d.join("w.h"), "template <class T> int f(T) { int unused; return 0; }\n")
        .unwrap();
    let o = ok(&d, &["-Wall", "-fkiln-pch-image", "-x", "c++-header", "w.h", "-o", "w.pch"]);
    assert!(o.stderr.contains("unused variable"), "{}", o.stderr);
    std::fs::write(d.join("w.cpp"), "int main() { return f(1); }\n").unwrap();
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-Wall", "-fkiln-pch-image=check", "-include-pch", "w.pch"];
        let o = ok(&d, &[&check[..], mode, &["w.cpp", "-o", "-"]].concat());
        assert!(!o.stderr.contains("differs"), "{mode:?}: {}", o.stderr);
        assert!(o.stderr.contains("unused variable"), "{mode:?}: {}", o.stderr);
    }
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

/// WP-1343: `header` is carried whole by the image (in C++26), and
/// `tu` compiles the same through it as through the replay, and
/// `check` agrees.
fn carried_1343(name: &str, header: &str, tu: &str) {
    let d = scratch(name);
    std::fs::write(d.join("a.h"), header).unwrap();
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    let std = "-std=c++26";
    let o = ok(&d, &[std, "-v", "-fkiln-pch-image", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("consumers use its declaration image"), "{}", o.stderr);
    let o = ok(
        &d,
        &[std, "-v", "-fkiln-pch-image=load", "-include-pch", "a.pch", "-fsyntax-only", "a.cpp"],
    );
    assert!(o.stderr.contains("': declaration image ("), "{}", o.stderr);
    let ir = ["-S", "-emit-llvm", "-O0", "a.cpp", "-o", "-"];
    let replay = ok(&d, &[&[std, "-include-pch", "a.pch"][..], &ir].concat());
    let image =
        ok(&d, &[&[std, "-fkiln-pch-image=load", "-include-pch", "a.pch"][..], &ir].concat());
    assert_eq!(replay.stderr, image.stderr);
    assert_eq!(replay.stdout, image.stdout);
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = [std, "-fkiln-pch-image=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// WP-1343: rewritten comparisons, delete-expressions, `typeid`,
/// `dynamic_cast`, `__builtin_bit_cast`, atomic builtins, array type
/// traits and qualified member names in bodies are carried.
#[test]
fn body_expression_forms_are_carried() {
    let header = "\
namespace std { class type_info { public: virtual ~type_info(); const char* name() const noexcept; }; }
struct P { int a; constexpr bool operator==(const P&) const = default; };
constexpr bool ne(P x, P y) { return x != y; }
struct B { virtual ~B() {} int b = 1; };
struct D : B { int d = 2; };
inline int dc(B* p) { D* q = dynamic_cast<D*>(p); return q ? q->d : -1; }
inline bool same(B& r) { return &typeid(r) == &typeid(D) && &typeid(int) != &typeid(long); }
inline void del(int* p, int* a) { delete p; delete[] a; }
inline float bc(unsigned u) { return __builtin_bit_cast(float, u); }
inline int at(int* p) { return __atomic_load_n(p, __ATOMIC_SEQ_CST) + __atomic_fetch_add(p, 1, __ATOMIC_RELAXED); }
inline int ext() { return int(__array_extent(int[3][4], 1)) + int(__array_rank(int[2][2])); }
struct Base { int m() const { return 5; } };
struct Der : Base { int m() const { return Base::m() + this->Base::m(); } };
";
    let tu = "int main() { D d; int v = 4; del(new int(1), new int[2]);
  return ne(P{1}, P{2}) && dc(&d) == 2 && same(d) && bc(0x3f800000u) == 1.0f && at(&v) == 8
    && ext() == 6 && Der{}.m() == 10 ? 0 : 1; }\n";
    carried_1343("wp1343e", header, tu);
}

/// WP-1343: try blocks, and block-scope using-declarations,
/// using-directives, using-enum-declarations and namespace aliases.
#[test]
fn try_blocks_and_local_usings_are_carried() {
    let header = "\
namespace lib { inline int f(int x) { return x + 1; } inline namespace v1 { inline int g() { return 7; } } }
inline int lu() { using lib::f; using namespace lib; return f(1) + g(); }
inline int tr(int x) { try { if (x) throw x; return 0; } catch (int e) { return e; } catch (...) { return -2; } }
enum class Col { red, green };
inline int ue(Col c) { using enum Col; namespace L = lib; return c == green ? L::f(0) : 0; }
";
    let tu = "int main() { return lu() == 9 && tr(3) == 3 && ue(Col::green) == 1 ? 0 : 1; }\n";
    carried_1343("wp1343t", header, tu);
}

/// WP-1343: block-scope structured bindings (of a class and by
/// reference to an array, and in a template's instantiation) and union
/// initializers.
#[test]
fn structured_bindings_and_union_inits_are_carried() {
    let header = "\
union U { int i; float f; };
inline int un() { U u{5}; return u.i; }
struct Pt { int x, y; };
inline int sb() { Pt p{3, 4}; auto [a, b] = p; int arr[2] = {5, 6}; auto& [c, d] = arr; return a * b + c + d; }
template <class T> int tsb(T t) { auto [u, v] = t; return u - v; }
inline int usb() { return tsb(Pt{9, 2}); }
";
    let tu = "int main() { auto [p, q] = Pt{1, 2}; return un() == 5 && sb() == 23 && usb() == 7 && p + q == 3 ? 0 : 1; }\n";
    carried_1343("wp1343b", header, tu);
}

/// WP-1343: pointer and class constants in bodies (an immediate
/// invocation's result), `_Atomic` types and a constrained placeholder.
#[test]
fn constants_atomic_and_placeholder_types_are_carried() {
    let header = "\
struct Pt { int x, y; };
consteval Pt mk() { return {1, 2}; }
inline int ag() { return mk().x + mk().y; }
inline constexpr int gv = 4;
consteval const int* gp() { return &gv; }
inline int pc() { return *gp(); }
inline int atm() { _Atomic(int) a = 3; return a; }
template <class T> concept Any = true;
inline Any auto cr() { return 3; }
inline decltype(auto) dr(int& r) { return (r); }
";
    let tu = "int main() { int k = 1; dr(k) = 2; return ag() == 3 && pc() == 4 && atm() == 3 && cr() == 3 && k == 2 ? 0 : 1; }\n";
    carried_1343("wp1343c", header, tu);
}

/// WP-1343 remaining: a default argument used on a call of a function
/// template specialization names the specialization's parameter, which
/// has no record (`bits/fs_path.h`'s `string<char>()`); the header is
/// replayed, with the reason.
#[test]
fn specialization_default_argument_is_refused() {
    let d = scratch("wp1343p");
    let header = "\
template <class T> T sq(T x, T k = T(2)) { return x * k; }
inline int use() { return sq<int>(3); }
";
    let note = produce(&d, "a", header);
    assert!(note.contains("a specialization's parameter"), "{note}");
}

/// Reduced from `cpp.include/pch-body-forms-errors.cpp` under `--pch-diff
/// check` (found by WP-1343, not fixed; pre-existing: the 1.1.0 release
/// candidate reproduces it under `-fkiln-pch-image=check` with
/// `-fsyntax-only` or `-S -emit-llvm`, at every `-std`): a header's
/// polymorphic class with a derived class, in a unit that never names
/// either, has the base destructor's exception specification evaluated
/// by the replay (`'void () noexcept'`) and still unevaluated from the
/// image. The fix resolves the implicit `noexcept` the derived class's
/// completion needs ([except.spec]/8) while the importer builds it.
#[test]
#[ignore = "pre-existing (1.1.0): image leaves a base destructor's noexcept unevaluated"]
fn virtual_dtor_noexcept_matches_replay() {
    let d = scratch("wp1343v");
    produce(&d, "a", "struct B { virtual ~B() {} };\nstruct D : B { int d = 2; };\n");
    std::fs::write(d.join("a.cpp"), "int main() { return 0; }\n").unwrap();
    let check = ["-fkiln-pch-image=check", "-include-pch", "a.pch", "-fsyntax-only", "a.cpp"];
    assert_eq!(ok(&d, &check).stderr, "");
}

/// WP-1345: dependent using-declarations in class templates
/// ([namespace.udecl]/1: `using B<T>::name;`, `using typename
/// B<T>::type;`, an operator) are carried, and each instantiation
/// redoes them.
#[test]
fn dependent_using_declarations_are_carried() {
    let header = "\
template <class T> struct B { static const bool value = sizeof(T) > 1; typedef T type;
  static int f(int x) { return x + 1; } int g() const { return 2; } };
template <class T> struct D : B<T> {
  using B<T>::value;
  using typename B<T>::type;
  using B<T>::f;
  using B<T>::g;
  enum { width = value ? sizeof(T) * 8 : 0 };
  static type h(type t) { return f(t); }
  int k() const { return g() + 1; }
};
struct Op { int operator()(int x) const { return x; } bool operator==(const Op&) const { return true; } };
template <class T> struct F : T { using T::operator(); using T::operator==; };
";
    let tu = "int main() { D<int> d; F<Op> o;
  return D<int>::width == 32 && D<int>::h(2) == 3 && d.k() == 3 && o(4) == 4 ? 0 : 1; }\n";
    carried_1343("wp1345u", header, tu);
}

/// WP-1345: `preferred_name`, `diagnose_if`, `enable_if` and
/// `alignas(type)` are carried: the importer's printing, overload
/// resolution, warnings and errors and layout are the replay's.
#[test]
fn preferred_name_diagnose_if_and_enable_if_are_carried() {
    let header = "\
template <class C> struct bs;
using str = bs<char>;
template <class C> struct [[clang::preferred_name(str)]] bs { C c; };
inline int f(int n) __attribute__((enable_if(n > 0, \"positive\"))) { return 1; }
inline int f(int n) { return 2; }
inline void g(int n) __attribute__((diagnose_if(n < 0, \"negative\", \"warning\"))) {}
template <class T> struct V { void at(int n) const __attribute__((diagnose_if(n < 0, \"neg\", \"warning\"))) {} };
struct alignas(double) AD { char c; };
template <class T> struct alignas(T) AT { char c; };
";
    let tu = "int main() { g(1); V<int>{}.at(2); bs<char> s{}; (void)s;
  static_assert(alignof(AD) == alignof(double) && alignof(AT<long double>) == alignof(long double));
  return f(1) + f(-1) - 3; }\n";
    carried_1343("wp1345a", header, tu);
    // The attribute's diagnostic, and an error naming the specialization,
    // are the replay's (clang++ 19 and 22 print the same).
    let d = scratch("wp1345a2");
    std::fs::write(d.join("a.h"), header).unwrap();
    std::fs::write(
        d.join("a.cpp"),
        "template <class T> struct hold { T t; };\nint main() { g(-1); hold<bs<char>> s; int x = s; return x; }\n",
    )
    .unwrap();
    ok(&d, &["-std=c++26", "-fkiln-pch-image", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    let args = ["-std=c++26", "-fkiln-pch-image=check", "-include-pch", "a.pch", "-fsyntax-only"];
    let o = kiln(&d, &[&args[..], &["a.cpp"]].concat());
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("warning: negative"), "{}", o.stderr);
    assert!(
        o.stderr.contains("no viable conversion from 'hold<bs<char>>' to 'int'"),
        "{}",
        o.stderr
    );
    assert!(!o.stderr.contains("differs"), "{}", o.stderr);
}

/// WP-1345: an implicit special member of a specialization the header
/// used (the default constructor and destructor of `holder<S>`, which
/// an inline variable's initialization names) is carried and defined
/// again.
#[test]
fn implicit_members_of_specializations_are_carried() {
    let header = "\
struct S { int v = 7; ~S() {} };
template <class T> struct holder { T t; int n; };
inline holder<S> hs;
inline int get() { holder<S> h; h.n = 2; return h.t.v + h.n; }
";
    let tu = "int main() { return hs.t.v + get() - 16; }\n";
    carried_1343("wp1345i", header, tu);
}

/// WP-1345: `explicit(expr)` ([dcl.fct.spec]/3) on constructors and
/// conversion functions, of a class template's members (substituted by
/// each instantiation) and of a class.
#[test]
fn explicit_specifiers_are_carried() {
    let header = "\
template <class T> constexpr bool small = sizeof(T) < 4;
template <class T> struct W {
  T v;
  explicit(small<T>) W(T x) : v(x) {}
  template <class U> explicit(!small<U>) W(U*, U u) : v(T(u)) {}
  explicit(sizeof(T) > 1) operator bool() const { return v != 0; }
};
struct N { explicit(true) N(int) {} explicit(false) N(long) {} };
";
    let tu = "\
template <class T, class A> concept conv = requires(A a) { [](T) {}(a); };
static_assert(!conv<W<char>, char> && conv<W<long>, long> && conv<N, long>);
int main() { W<char> c(char(1)); bool b = c; W<long> l = 3L; return int(l.v) + b - 5 + (bool)l; }
";
    carried_1343("wp1345x", header, tu);
}

/// WP-1345: conversion function templates, literal operators and
/// literal operator templates are named in the image.
#[test]
fn conversion_templates_and_literal_operators_are_carried() {
    let header = "\
struct M { long v;
  template <class T> requires (sizeof(T) >= 4) constexpr explicit operator T() const { return T(v); }
  template <class T> operator T*() const { return nullptr; } };
namespace lits {
inline namespace sv { constexpr unsigned long long operator\"\"_k(unsigned long long n) { return n * 1000; } }
template <char... C> constexpr int operator\"\"_n() { return sizeof...(C); }
}
";
    let tu = "using namespace lits;
int main() { M m{5}; int* p = m; return int(m) + (p ? 1 : 0) + int(2_k / 1000) + 123_n - 10; }\n";
    carried_1343("wp1345c", header, tu);
}

/// WP-1345: abbreviated function templates ([dcl.fct]/22): their
/// invented template parameters are carried.
#[test]
fn abbreviated_function_templates_are_carried() {
    let header = "\
struct F {
  void operator()(auto&&) const = delete;
  template <class T> int operator()(T* p) const { return *p; }
  int twice(const auto& x, auto... ys) const { return int(x) * 2 + int(sizeof...(ys)); }
};
inline constexpr F f{};
inline int plus1(auto x) { return int(x) + 1; }
template <class T> struct B { static int g(T t, auto u) { return int(t) + int(u); } };
";
    let tu = "template <class X> concept callable = requires(X x) { f(x); };
static_assert(!callable<int> && callable<int*>);
int main() { int v = 3; return f(&v) + f.twice(1.5, 'a', 2) + plus1(2.0) + B<int>::g(1, 2L) - 13; }\n";
    carried_1343("wp1345t", header, tu);
}

/// WP-1345: a class template pattern's explicitly defaulted copy
/// constructor whose class has a non-type parameter (sema leaves its
/// special kind to the instantiations), and an explicit specialization
/// a declaration names before it is declared (`plus<void>` in a
/// variable template partial specialization's arguments, then
/// `template <> struct plus<void> { ... }`, its members after them).
#[test]
fn pattern_defaults_and_late_explicit_specializations_are_carried() {
    let header = "\
template <bool S, class Tag = void> struct EDC {
  int v = 1;
  constexpr EDC() noexcept = default;
  constexpr EDC(EDC const&) noexcept = default;
};
template <class T = void> struct pl { T operator()(T a, T b) const { return a + b; } };
template <class Tag, class Op, class A, class B> inline const bool desugars = false;
struct plus_tag {};
template <class A, class B> inline const bool desugars<plus_tag, pl<void>, A, B> = true;
template <> struct pl<void> {
  template <class A, class B> auto operator()(A a, B b) const { return a + b; }
  typedef void is_transparent;
};
";
    let tu = "int main() { EDC<true> e; EDC<true> f = e;
  static_assert(desugars<plus_tag, pl<void>, int, long> && !desugars<plus_tag, pl<int>, int, int>);
  return pl<>{}(1, 2L) + pl<int>{}(f.v, 1) - 5; }\n";
    carried_1343("wp1345d", header, tu);
}

/// WP-1345: anonymous unions and structs in classes and class template
/// patterns ([class.union.anon]): the member object and the injected
/// member names, mem-initializers naming them, and member access.
#[test]
fn anonymous_class_members_are_carried() {
    let header = "\
struct U {
  union { char c; int v; };
  struct { short a, b; };
  U() : v(5) {}
  explicit U(char x) : c(x) {}
  int get() const { return v + a * 0; }
};
template <class T> struct Opt {
  union { char null_; T val_; };
  bool engaged_;
  constexpr Opt() : null_(), engaged_(false) {}
  constexpr Opt(T t) : val_(t), engaged_(true) {}
  constexpr T value_or(T d) const { return engaged_ ? val_ : d; }
};
class P { int x = 1; public: union { long l; double d; }; };
";
    let tu = "static_assert(Opt<int>(3).value_or(1) == 3 && Opt<int>().value_or(4) == 4);
int main() { U u; U w('a'); P p; p.l = 2; Opt<long> o(7);
  return u.get() + (w.c == 'a') + int(p.l) + int(o.value_or(0)) - 15; }\n";
    carried_1343("wp1345n", header, tu);
}

/// The directory [`carried_1343`] made for `name` (not emptied).
fn scratch_of(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kiln-pch-decls-{}-{name}", std::process::id()))
}

/// WP-1345: a dependent inheriting constructor (`using B<T>::B;`,
/// [class.inhctor.init]) in a class template, and a dependent
/// using-declaration of a base's member next to it.
#[test]
fn dependent_inheriting_constructors_are_carried() {
    let header = "\
template <class T> struct B { B(int) {} B(T, T) {} int v = 0; };
template <class T> struct D : B<T> { using B<T>::B; };
template <class T> struct E : D<T> { using D<T>::D; using D<T>::v; };
template <class T, bool = true> struct pay;
template <class T> struct pay<T, true> : B<T> { using B<T>::B; };
";
    let tu = "D<int> d(1);\nE<long> e(2L, 3L);\npay<int> p(4);\nint main() { return d.v + e.v + p.v; }\n";
    carried_1343("wp1345o", header, tu);
}

/// WP-1345: a using-enum-declaration in a class ([enum.udecl]) and the
/// enumerator shadows it introduces, with their access; a block-scope
/// function declaration in a template's member; a namespace-scope
/// `asm` declaration ([dcl.asm]).
#[test]
fn class_using_enum_local_functions_and_asm_are_carried() {
    let header = "\
struct te {
  enum class id : int { a = 1, b, c };
  using enum id;
  int f() const { return int(b); }
private:
  enum class q { x = 7 };
  using enum q;
public:
  static int g() { return int(x); }
};
struct mr { int k = 6; };
inline mr* def() noexcept { static mr m; return &m; }
template <class T> struct pa {
  mr* r;
  pa() noexcept { extern mr* def() noexcept __attribute__((__returns_nonnull__)); r = def(); }
};
inline int plain() { extern mr* def() noexcept; return def()->k; }
asm(\".globl wp1345_sym\\n.set wp1345_sym, 0\");
";
    let tu = "int main() { te t; pa<int> p;
  return t.f() + int(te::c) + te::g() + p.r->k + plain() - 24; }\n";
    carried_1343("wp1345p", header, tu);
    // The private enumerator stays private.
    let d = scratch_of("wp1345p");
    std::fs::write(d.join("b.cpp"), "int z = int(te::x);\n").unwrap();
    let args = ["-std=c++26", "-fkiln-pch-image=check", "-include-pch", "a.pch", "-fsyntax-only"];
    let o = kiln(&d, &[&args[..], &["b.cpp"]].concat());
    assert!(o.stderr.contains("error: 'x' is a private member of 'te'"), "{}", o.stderr);
    assert!(!o.stderr.contains("differs"), "{}", o.stderr);
}

/// WP-1345: out-of-line members of classes nested in class templates
/// and partial specializations, an out-of-line partial specialization
/// of a member class template, and explicit object member functions
/// ([dcl.fct]/6).
#[test]
fn nested_out_of_line_members_and_explicit_object_parameters_are_carried() {
    let header = "\
template <class C> struct os {
  class sentry;
  struct in { int f(); static int s; template <class U> U g(U); };
  int v = 3;
};
template <class C> class os<C>::sentry {
  bool ok;
public:
  explicit sentry(os& o);
  ~sentry();
  explicit operator bool() const { return ok; }
};
template <class C> os<C>::sentry::sentry(os& o) : ok(o.v == 3) {}
template <class C> os<C>::sentry::~sentry() {}
template <class C> int os<C>::in::f() { return 4; }
template <class C> int os<C>::in::s = 5;
template <class C> template <class U> U os<C>::in::g(U u) { return u + 1; }
template <class C, class D> struct p3 {};
template <class C> struct p3<C, int> { struct in { int f(); }; };
template <class C> int p3<C, int>::in::f() { return sizeof(C); }
struct fb {
  struct rb { int k = 1; };
  template <class R> struct res : rb { R r{}; };
};
template <class R> struct fb::res<R&> : fb::rb { R* p = nullptr; int g() { return 2; } };
struct S {
  int v = 4;
  int get(this const S& s) { return s.v; }
  template <class Self> auto&& ref(this Self&& self) { return static_cast<Self&&>(self).v; }
};
template <class T> struct W {
  T t;
  template <class Self> constexpr T val(this Self&& self) { return self.t + 1; }
  T plain(this W w);
};
template <class T> T W<T>::plain(this W w) { return w.t * 2; }
";
    let tu = "int main() {
  os<char> o; os<char>::sentry s(o); os<char>::in i; p3<char, int>::in q;
  fb::res<int&> x; fb::res<int> y; S z; W<int> w{3}; z.ref() = 5;
  static_assert(W<int>{1}.val() == 2);
  return (bool(s) ? 0 : 1) + i.f() + os<char>::in::s + int(i.g(1L)) + q.f() + x.g() + x.k
    + y.r + z.get() + w.val() + w.plain() - 29; }\n";
    carried_1343("wp1345q", header, tu);
}

/// WP-1345: `x.operator T()` in a dependent member access, attributes
/// written on explicit instantiations, and a specialization with
/// `explicit(expr)` members the header completed: copy-initialization
/// still sees which constructors are explicit, and the implicit members
/// are declared in replay's order (the candidate notes match).
#[test]
fn conversion_names_instantiation_attributes_and_explicit_specs_are_carried() {
    let header = "\
template <class T> struct sv {};
template <class R, class T>
constexpr bool has_conv = requires (R& r) { r.operator sv<T>(); };
struct A { operator sv<int>() const { return {}; } };
template <class T> struct [[gnu::visibility(\"default\")]] sb { T v{}; T get() const { return v; } };
template <class T> constexpr bool imp = sizeof(T) > 1;
template <class A, class B> struct pr {
  A a; B b;
  constexpr pr() : a(), b() {}
  template <class U, class V> explicit(!imp<U>) constexpr pr(U u, V v) : a(u), b(v) {}
  explicit(sizeof(A) == 1) constexpr pr(A x) : a(x) {}
};
inline pr<bool, long> need(long) { pr<bool, long> p(true); p.b = 3; return p; }
// Last: the importer runs explicit instantiations after the records
// (WP-1307), so `check` sees a later declaration move before it.
template struct __attribute__((__visibility__(\"hidden\"))) sb<int>;
";
    let tu = "static_assert(has_conv<A, int> && !has_conv<A, long>);
pr<bool, long> f() { return {1, 2L}; }
int main() { sb<int> b; return b.get() + int(need(0).b) + int(f().b) - 5; }\n";
    carried_1343("wp1345r", header, tu);
    let d = scratch_of("wp1345r");
    std::fs::write(d.join("b.cpp"), "pr<bool, long> z = true;\npr<bool, long> w = {'c', 2};\n")
        .unwrap();
    let args = ["-std=c++26", "-fkiln-pch-image=check", "-include-pch", "a.pch", "-fsyntax-only"];
    let o = kiln(&d, &[&args[..], &["b.cpp"]].concat());
    assert!(
        o.stderr.contains("no viable conversion from 'bool' to 'pr<bool, long>'"),
        "{}",
        o.stderr
    );
    assert!(
        o.stderr.contains("chosen constructor is explicit in copy-initialization"),
        "{}",
        o.stderr
    );
    assert!(!o.stderr.contains("differs"), "{}", o.stderr);
}
