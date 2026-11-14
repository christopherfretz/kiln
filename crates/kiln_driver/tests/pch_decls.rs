//! A PCH consumer through `kiln++` against the replay of the same PCH
//! (spec/pch.md "Consumer"): the snapshot restore (the default) must
//! behave as the `#include` does ([cpp.include]/2): the same diagnostics,
//! byte for byte, and the same IR, including the header's definitions the
//! unit odr-uses ([basic.def.odr]). `-fkiln-pch-snapshot=check` restores
//! and replays in one compile and compares the two.
//!
//! WP-1397: these cases covered the retired declaration-image PCH
//! (WP-1036 to WP-1384); each is kept where the snapshot must get the same
//! behaviour right, now run as restore against replay.
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

/// The line `KILN_SNAP_TIMING` prints when a consumer restored the
/// snapshot.
const RESTORED: &str = "snapshot-timing: restore ";

/// Runs `kiln++` with `KILN_SNAP_TIMING` set; `restored` says whether a
/// consumer restored the snapshot, and the `snapshot-timing:` lines are
/// left out of `stderr`.
fn run(dir: &Path, args: &[&str]) -> (Out, bool) {
    let o = Command::new(kilnpp())
        .args(args)
        .env("KILN_SNAP_TIMING", "1")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let all = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!all.contains("panicked"), "kiln++ {args:?} panicked:\n{all}");
    let restored = all.lines().any(|l| l.starts_with(RESTORED));
    let mut stderr = String::new();
    for l in all.split_inclusive('\n').filter(|l| !l.starts_with("snapshot-timing:")) {
        stderr.push_str(l);
    }
    let out = Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    };
    (out, restored)
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    run(dir, args).0
}

fn ok(dir: &Path, args: &[&str]) -> Out {
    let o = kiln(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    o
}

/// Builds `<name>.pch` from `<name>.h`, returning the producer's `-v`
/// notes (`...: snapshot written` when it carries the snapshot).
fn produce(dir: &Path, name: &str, header: &str) -> String {
    produce_with(dir, name, header, &[])
}

/// [`produce`] with `flags` too: a consumer restores only a PCH built
/// with its own configuration (spec/pch.md "Identity"), so a consumer
/// with `-Wall` or `-O1` needs a PCH built with them.
fn produce_with(dir: &Path, name: &str, header: &str, flags: &[&str]) -> String {
    std::fs::write(dir.join(format!("{name}.h")), header).unwrap();
    let (h, pch) = (format!("{name}.h"), format!("{name}.pch"));
    let o = ok(dir, &[flags, &["-v", "-x", "c++-header", &h, "-o", &pch]].concat());
    o.stderr.lines().filter(|l| l.starts_with("note: precompiled header")).collect()
}

/// Compiles `tu` with the PCH replayed (`-fno-kiln-pch-snapshot`) and
/// restored (the default, which must restore), and asserts the exit code,
/// the diagnostics and the output are the same; returns the restore's.
fn same(dir: &Path, pch: &str, mode: &[&str], tu: &str) -> Out {
    let replay = ["-fno-kiln-pch-snapshot", "-include-pch", pch];
    let replay = kiln(dir, &[&replay[..], mode, &[tu, "-o", "-"]].concat());
    let (restore, restored) =
        run(dir, &[&["-include-pch", pch][..], mode, &[tu, "-o", "-"]].concat());
    assert!(restored, "{mode:?}: the consumer did not restore the snapshot:\n{}", restore.stderr);
    assert_eq!(replay.code, restore.code, "{mode:?}:\n{}\n---\n{}", replay.stderr, restore.stderr);
    assert_eq!(replay.stderr, restore.stderr, "{mode:?}");
    assert_eq!(replay.stdout, restore.stdout, "{mode:?}");
    restore
}

/// The `-v` note of a consumer of `pch`.
fn path_note(dir: &Path, pch: &str, tu: &str) -> String {
    let o = kiln(dir, &["-v", "-include-pch", pch, "-fsyntax-only", tu]);
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

/// A restored compile matches replay: diagnostics, the IR (every
/// definition from the header the unit uses is emitted) and an object.
#[test]
fn snapshot_matches_replay() {
    let d = scratch("rich");
    let note = produce(&d, "r", RICH);
    assert!(note.contains("': snapshot written"), "{note}");
    std::fs::write(d.join("r.cpp"), RICH_TU).unwrap();
    assert!(path_note(&d, "r.pch", "r.cpp").contains("': snapshot matches; restoring it"));
    assert!(produce_with(&d, "rw", RICH, &["-Wall"]).contains("': snapshot written"));
    same(&d, "rw.pch", &["-fsyntax-only", "-Wall"], "r.cpp");
    let ir = same(&d, "r.pch", &["-S", "-emit-llvm", "-O0"], "r.cpp");
    for def in ["@_Z6defargii(", "@_Z3strv(", "@_ZN5outer2v13inlEv(", "@_ZL2sti("] {
        assert!(ir.stdout.lines().any(|l| l.starts_with("define") && l.contains(def)), "{def}");
    }
    for var in ["@_ZN2SD1xE = ", "@tentative = ", "@_ZL8internal = ", "@ev = "] {
        assert!(ir.stdout.contains(var), "{var}");
    }
    assert!(produce_with(&d, "r1", RICH, &["-O1"]).contains("': snapshot written"));
    same(&d, "r1.pch", &["-S", "-emit-llvm", "-O1"], "r.cpp");
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
/// after it ([cpp.pragma]).
#[test]
fn pragma_state_crosses_the_boundary() {
    let d = scratch("pragmas");
    let header = "struct P { char c; int i; };\n#pragma pack(push, 1)\n\
                  #pragma clang diagnostic push\n#pragma clang diagnostic ignored \"-Wunused-value\"\n";
    assert!(produce(&d, "p", header).contains("': snapshot written"));
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
    assert!(path_note(&d, "p.pch", "p.cpp").contains("': snapshot matches; restoring it"));
    let o = same(&d, "p.pch", &["-S", "-emit-llvm"], "p.cpp");
    assert_eq!(o.stderr.matches("warning: expression result unused").count(), 1, "{}", o.stderr);
    assert!(o.stderr.contains("p.cpp:9:11"), "{}", o.stderr);
}

/// A local class a deduced return type names (the image could not carry
/// it) crosses the PCH ([dcl.spec.auto]).
#[test]
fn local_class_return_type_crosses() {
    let d = scratch("fallback");
    let note = produce(&d, "l", "inline auto l() { struct R { int q = 0; }; return R{}; }\n");
    assert!(note.contains("': snapshot written"), "{note}");
    std::fs::write(d.join("l.cpp"), "int main() { return l().q; }\n").unwrap();
    let note = path_note(&d, "l.pch", "l.cpp");
    assert!(note.contains("': snapshot matches; restoring it"), "{note}");
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
    assert!(note.contains("': snapshot written"), "{note}");
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
    assert!(note.contains("': snapshot written"), "{note}");
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
    assert!(path_note(&d, "x.pch", "x.cpp").contains("': snapshot matches; restoring it"));
    assert!(produce_with(&d, "xw", header, &["-Wall"]).contains("': snapshot written"));
    same(&d, "xw.pch", &["-fsyntax-only", "-Wall"], "x.cpp");
    same(&d, "x.pch", &["-S", "-emit-llvm", "-O0"], "x.cpp");
    let check = ["-fkiln-pch-snapshot=check", "-include-pch", "x.pch", "-S", "-emit-llvm"];
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

/// Sema state the consumer depends on (a pending `#pragma weak` naming a
/// function the unit defines) crosses the PCH: `f` is weak (the retired
/// declaration image could not carry it and made `f` strong).
#[test]
fn pending_pragma_weak_crosses() {
    let d = scratch("sema-state");
    let note = produce(&d, "w", "#pragma weak f\nextern \"C\" int g(int);\n");
    assert!(note.contains("': snapshot written"), "{note}");
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
fn local_static_asserts_and_explicit_args_cross() {
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
    assert!(note.contains("': snapshot written"), "{note}");
    std::fs::write(
        d.join("a.cpp"),
        "int main() { return f(3) + g(2) + S<int>::h() + callpick() + int(td(1)); }\n",
    )
    .unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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
fn local_externs_cross() {
    let d = scratch("local-extern");
    let header = "inline int g() { extern int v; return v; }\n\
                  namespace n { inline void t() { void k(); k(); } }\nnamespace n { void k(); }\n\
                  namespace n { inline namespace i { inline long u() { extern long w; return w; } } }\n\
                  inline int h() { int f(int = 2); return f(); }\n";
    let note = produce(&d, "y", header);
    assert!(note.contains("': snapshot written"), "{note}");
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
    let check = ["-fkiln-pch-snapshot=check", "-include-pch", "y.pch", "-S", "-emit-llvm"];
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
    assert!(note.contains("': snapshot written"), "{note}");
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
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    let ir = same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    assert!(ir.stdout.contains("cold noreturn"), "{}", ir.stdout);
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "\
int main() {
  int *p = new int(3); int r = *p; delete p;
  alignas(int) unsigned char b[sizeof(int)]; r += *::new (b) int(4);
  Pos<long> a(1); r += int((a + 2).off);
  return r - 10;
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    let ir = same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    assert!(ir.stdout.contains("@_Znwm"), "{}", ir.stdout);
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "\
int main() {
  Promote<int>::type x = 1.5; Promote<float>::type y = 2.5f;
  return int(x + y) - 4 + int(sizeof(Promote<long double>::type) != sizeof(long double));
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
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
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "\
struct S {};
int main() {
  Hold<long> h;
  return (Is<int>::value ? 0 : 1) + (Is<S>::value ? 2 : 0) + (sizeof(h.b) == sizeof(long) ? 0 : 4);
}
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// WP-1342: `header` carries a snapshot, and `tu` compiled against it
/// agrees with replay (`same`, `-fkiln-pch-snapshot=check`) for syntax
/// and IR, and runs to exit code 0.
fn snapshot_agrees(name: &str, header: &str, tu: &str) {
    let d = scratch(name);
    std::fs::write(d.join("a.h"), header).unwrap();
    let o = ok(&d, &["-std=c++20", "-v", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    let load = ["-std=c++20", "-include-pch", "a.pch"];
    let o = ok(&d, &[&load[..], &["-v", "-fsyntax-only", "a.cpp"]].concat());
    assert!(o.stderr.contains("': snapshot matches; restoring it"), "{}", o.stderr);
    same(&d, "a.pch", &["-std=c++20", "-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-std=c++20", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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
    snapshot_agrees("wp1342a", header, tu);
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
    snapshot_agrees("wp1342b", header, tu);
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
    snapshot_agrees("wp1342c", header, tu);
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
    snapshot_agrees("wp1342d", header, tu);
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
    snapshot_agrees("wp1344a", header, tu);
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
    snapshot_agrees("wp1344b", header, tu);
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
    snapshot_agrees("wp1344c", header, tu);
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
    snapshot_agrees("wp1344d", header, tu);
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
    snapshot_agrees("wp1344e", header, tu);
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
    snapshot_agrees("wp1344f", header, tu);
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
    snapshot_agrees("wp1344g", header, tu);
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
    snapshot_agrees("wp1344h", header, tu);
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
    snapshot_agrees("wp1344i", header, tu);
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
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "\
static_assert(sz<int>() == 8 && nt<int>);
int main() { return sz<char>() + plain() == 5 ? 0 : 1; }
";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "int main() { int a[2] = {1, 2}; short k[3] = {0, 0, 3}; return first2(a) + H<int>{}.at(k) == 6 ? 0 : 1; }\n";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "int main() { Box<int> b{twice(3.0)}; return b.re() == 6 && __imag__ mk(1, 2) == 2 ? 0 : 1; }\n";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    assert!(path_note(&d, "a.pch", "a.cpp").contains("': snapshot matches; restoring it"));
    same(&d, "a.pch", &["-S", "-emit-llvm", "-O0"], "a.cpp");
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// Reduced from `module.global.frag/gmf-floatn-limits.cpp` under the
/// retired image's `--pch-diff check` (WP-1320 deliverable 5, fixed by
/// WP-1353): an imported module's GMF that includes the header the
/// importer restored from the PCH is skipped by position (spec/modules.md
/// §6), as after a replay, instead of re-declaring `size_t` and
/// redefining `lim`.
#[test]
fn gmf_reinclusion_merges_with_restored_declarations() {
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
    let o = ok(&d, &["-std=c++20", "-v", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
    let m = "module;\n#include \"a.h\"\nexport module m;\nexport constexpr int get() { return lim<float>::d == 24; }\n";
    std::fs::write(d.join("m.cppm"), m).unwrap();
    ok(&d, &["-std=c++20", "--precompile", "-x", "c++-module", "m.cppm", "-o", "m.pcm"]);
    std::fs::write(d.join("t.cpp"), "import m;\nstatic_assert(get() == 1);\nint main() {}\n")
        .unwrap();
    let check = [
        "-std=c++20",
        "-fmodule-file=m=m.pcm",
        "-fkiln-pch-snapshot=check",
        "-include-pch",
        "a.pch",
    ];
    let o = kiln(&d, &[&check[..], &["-fsyntax-only", "t.cpp"]].concat());
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

/// A header template's definition-time warning is repeated by the replay
/// and not by the restore (spec/pch.md "Diagnostics from the prefix");
/// `check` accepts that (WP-1320).
#[test]
fn check_accepts_header_warnings() {
    let d = scratch("wp1320w");
    std::fs::write(d.join("w.h"), "template <class T> int f(T) { int unused; return 0; }\n")
        .unwrap();
    let o = ok(&d, &["-Wall", "-x", "c++-header", "w.h", "-o", "w.pch"]);
    assert!(o.stderr.contains("unused variable"), "{}", o.stderr);
    std::fs::write(d.join("w.cpp"), "int main() { return f(1); }\n").unwrap();
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-Wall", "-fkiln-pch-snapshot=check", "-include-pch", "w.pch"];
        let o = ok(&d, &[&check[..], mode, &["w.cpp", "-o", "-"]].concat());
        assert!(!o.stderr.contains("differs"), "{mode:?}: {}", o.stderr);
        assert!(o.stderr.contains("unused variable"), "{mode:?}: {}", o.stderr);
    }
}

/// `check` restores the snapshot and replays, and finds them equal.
#[test]
fn check_runs_both() {
    let d = scratch("check");
    produce(&d, "r", RICH);
    std::fs::write(d.join("r.cpp"), RICH_TU).unwrap();
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = ["-fkiln-pch-snapshot=check", "-include-pch", "r.pch"];
        let o = ok(&d, &[&check[..], mode, &["r.cpp", "-o", "-"]].concat());
        assert_eq!(o.stderr, "", "{mode:?}");
    }
}

/// A header edited after the PCH was built, its size kept, with the
/// fingerprint check off: the snapshot is not used (its text is not the
/// header's) and the edited header is replayed.
#[test]
fn stale_header_is_replayed() {
    let d = scratch("stale");
    produce(&d, "s", "inline int v() { return 7; }\n");
    std::fs::write(d.join("s.h"), "inline int v() { return 9; }\n").unwrap();
    std::fs::write(d.join("s.cpp"), "int main() { return v(); }\n").unwrap();
    let args = ["-v", "-Xclang", "-fno-validate-pch", "-include-pch"];
    let o = ok(&d, &[&args[..], &["s.pch", "-S", "-emit-llvm", "-o", "-", "s.cpp"]].concat());
    assert!(o.stderr.contains("'s.pch': snapshot not used: 's.h' changed"), "{}", o.stderr);
    assert!(o.stdout.contains("ret i32 9"), "{}", o.stdout);
}

/// A PCH whose section block is damaged is an error, never a guess.
#[test]
fn damaged_section_block_is_an_error() {
    let d = scratch("damaged");
    produce(&d, "x", "int x();\n");
    let mut b = std::fs::read(d.join("x.pch")).unwrap();
    let at = b.len() - 20;
    b[at] ^= 0xff;
    std::fs::write(d.join("x.pch"), b).unwrap();
    std::fs::write(d.join("x.cpp"), "int y = x();\n").unwrap();
    let o = kiln(&d, &["-include-pch", "x.pch", "-fsyntax-only", "x.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("malformed or corrupted precompiled file"), "{}", o.stderr);
}

/// WP-1343: `header` carries a snapshot (in C++26), and `tu` compiles
/// the same through it as through the replay, and `check` agrees.
fn carried_1343(name: &str, header: &str, tu: &str) {
    let d = scratch(name);
    std::fs::write(d.join("a.h"), header).unwrap();
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    let std = "-std=c++26";
    let o = ok(&d, &[std, "-v", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
    let o = ok(&d, &[std, "-v", "-include-pch", "a.pch", "-fsyntax-only", "a.cpp"]);
    assert!(o.stderr.contains("': snapshot matches; restoring it"), "{}", o.stderr);
    let ir = ["-S", "-emit-llvm", "-O0", "a.cpp", "-o", "-"];
    let replay = ["-fno-kiln-pch-snapshot", std, "-include-pch", "a.pch"];
    let replay = ok(&d, &[&replay[..], &ir].concat());
    let (restore, restored) = run(&d, &[&[std, "-include-pch", "a.pch"][..], &ir].concat());
    assert!(restored, "{}", restore.stderr);
    assert_eq!(replay.stderr, restore.stderr);
    assert_eq!(replay.stdout, restore.stdout);
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
        let check = [std, "-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
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

/// WP-1350: a default argument used on a call of a function template
/// specialization names the specialization's parameter
/// (`bits/fs_path.h`'s `string<char>()`); the image names it through
/// the specialization, the importer instantiates the used default again,
/// and the unit's own calls of the same specializations still find
/// their defaults.
#[test]
fn specialization_default_arguments_are_carried() {
    let header = "\
template <class T> T sq(T x, T k = T(2)) { return x * k; }
inline int use() { return sq<int>(3); }
struct Al { int id = 7; };
struct Str {
  int n = 0, a = 0;
  template <class It> Str(It b, It e, const Al& al = Al()) : n(int(e - b)), a(al.id) {}
};
inline Str mk(const char* s) { return Str(s, s + 3); }
";
    let tu = "int main() { const char* q = \"ab\"; Str s(q, q + 2);
  return use() + sq<int>(4) + mk(q).a + s.n == 6 + 8 + 7 + 2 ? 0 : 1; }\n";
    carried_1343("wp1350p", header, tu);
}

/// WP-1350: lambdas (captures, generic and pack lambdas, the conversion
/// to a function pointer, in a default member initializer), local
/// classes with virtual members, designated initializers in templates,
/// vectors of a dependent count, `__builtin_convertvector` and a member
/// typedef named in a member alias template come from the image.
#[test]
fn lambdas_local_classes_and_template_forms_are_carried() {
    let header = "\
inline int plain(int x) { auto f = [x, &y = x](int z) mutable { y += z; return x + z; }; int r = f(1); return r + x; }
inline int noncap() { int (*p)(int) = [](int y) { return y * 2; }; return p(4); }
template <class... T> int count(T... t) { return [&] { return (0 + ... + int(t)); }(); }
inline int generic() { auto g = [](auto a, auto b) { return a + b; }; return g(1, 2) + int(g(1.5, 2.5)); }
struct M { int (*fp)(int) = [](int a) { return a - 1; }; };
inline int lvirt() {
  struct B { virtual int f() { return 1; } virtual ~B() = default; };
  struct D : B { int f() override { return 2; } };
  D d; B& b = d; return b.f();
}
struct In { int c = 1; int d = 2; };
struct Out { int a = 0; In b; };
template <class T> Out mk(T x) { return Out{.a = int(x), .b = {.d = int(x) + 1}}; }
template <class T, unsigned long N> using vec __attribute__((__ext_vector_type__(N))) = T;
template <class T, unsigned long N> long widen(vec<T, N> v) {
  vec<long, N> w = __builtin_convertvector(v, vec<long, N>); return w[0] + w[N - 1];
}
template <class T, class D, bool = (sizeof(D) > 0)> struct ptr_of { using type = T*; };
template <bool B> struct en {};
template <> struct en<true> { using type = int; };
template <class T> struct up {
  using pointer = typename ptr_of<T, int>::type;
  pointer p = nullptr;
  template <class UP> using Same = typename en<__is_same(typename UP::pointer, pointer)>::type;
  template <class U, class = Same<up<U>>> int same() const { return 1; }
};
";
    let tu = "int main() { vec<int, 4> v = {1, 2, 3, 4};
  int r = plain(1) + noncap() + count(1, 2L, 'a') + generic() + M{}.fp(3) + lvirt() + mk(3).b.d +
          int(widen<int, 4>(v)) + up<int>{}.same<int>();
  return r == 4 + 8 + 100 + 7 + 2 + 2 + 4 + 5 + 1 ? 0 : 1; }\n";
    carried_1343("wp1350l", header, tu);
}

/// WP-1350 with WP-1355: a dependent vector type read back from the image
/// keeps its size expression as the canonical type's origin, so a
/// template of the header instantiated in the unit mangles `Dv <expr> _`
/// as under replay and as clang (`tests/mangle/dependent_vector.cpp`).
#[test]
fn dependent_vector_types_mangle_as_replayed() {
    let header = "\
template <class T, unsigned long N> using ev = T __attribute__((ext_vector_type(N)));
template <class T, unsigned long N>
using vs = T __attribute__((__vector_size__(N * sizeof(T))));
template <class T> using vs16 = T __attribute__((__vector_size__(16)));
template <class T, unsigned long N> bool a1(ev<T, N>) { return true; }
template <class T, unsigned long N> bool a2(vs<T, N>) { return true; }
template <class T> bool a4(vs16<T>) { return true; }
template <class T, unsigned long N> void b4(ev<T, N>, T *) {}
";
    let tu = "using v4 = int __attribute__((ext_vector_type(4)));
using w4 = int __attribute__((__vector_size__(16)));
int main() { v4 x{}; w4 y{}; b4<int, 4>(x, nullptr);
  return a1<int, 4>(x) && a2<int, 4>(y) && a4<int>(y) ? 0 : 1; }\n";
    carried_1343("wp1350v", header, tu);
    let d = scratch_of("wp1350v");
    let ir = ["-std=c++26", "-include-pch", "a.pch", "-S", "-emit-llvm"];
    let o = ok(&d, &[&ir[..], &["a.cpp", "-o", "-"]].concat());
    for name in [
        "@_Z2a1IiLm4EEbDvT0__T_",
        "@_Z2a2IiLm4EEbDvmlT0_stT__S0_",
        "@_Z2a4IiEbDvLi16E_T_",
        "@_Z2b4IiLm4EEvDvT0__T_PS0_",
    ] {
        assert!(o.stdout.contains(name), "{name}: {}", o.stdout);
    }
}

/// WP-1350: a dependent base named through a qualifier (`std::X<T>`,
/// first reached once `<compare>`'s lambdas were carried) and comparison
/// category values defined out of line, whose aggregate values the image
/// does not carry: the built-in `<=>` folds them from the initializers.
#[test]
fn qualified_dependent_base_and_comparison_categories_are_carried() {
    let header = "\
namespace q { template <class A, class B> struct pr { int v = 1; }; }
template <class T, class U> struct D : q::pr<T, U> {};
namespace std {
class partial_ordering {
  signed char _M_value;
  constexpr explicit partial_ordering(signed char v) noexcept : _M_value(v) {}
public:
  static const partial_ordering less, equivalent, greater, unordered;
  friend constexpr bool operator<(partial_ordering v, int) noexcept { return v._M_value == -1; }
};
inline constexpr partial_ordering partial_ordering::less(-1);
inline constexpr partial_ordering partial_ordering::equivalent(0);
inline constexpr partial_ordering partial_ordering::greater(1);
inline constexpr partial_ordering partial_ordering::unordered(2);
}
";
    let tu = "static_assert((1.0 <=> 2.0) < 0);
int main() { D<int, long> d; return d.v == 1 && (2.0 <=> 1.0) < 0 ? 1 : 0; }\n";
    carried_1343("wp1350c", header, tu);
}

/// Found by WP-1350 on its tree merged with WP-1353 (libc++ `<numeric>`,
/// `__numeric/gcd_lcm.h`; not fixed, no WP-1350 form involved): a type
/// naming a requires-expression parameter (`decltype(*i)` in a concept)
/// is built after every record (`exprs::validate_ty_texprs`), and types
/// are built in order, so a later body's local alias whose type is
/// written after it failed the image reader's check and the header was
/// replayed ("an internal image inconsistency"). The snapshot carries it,
/// and the consumer agrees with replay.
#[test]
fn local_alias_after_requires_parameter_type() {
    let d = scratch("wp1350q");
    let header = "template <class T> concept deref = requires(T i) { typename decltype(*i)::x; };
template <class T> struct mk { using type = unsigned long; };
template <class T> unsigned long g(T m) {
  using W = typename mk<T>::type;
  return static_cast<W>(m);
}
";
    std::fs::write(d.join("q.h"), header).unwrap();
    let o = ok(&d, &["-std=c++26", "-v", "-x", "c++-header", "q.h", "-o", "q.pch"]);
    assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
    std::fs::write(d.join("q.cpp"), "int main() { return int(g(0)); }\n").unwrap();
    same(&d, "q.pch", &["-std=c++26", "-S", "-emit-llvm"], "q.cpp");
}

/// WP-1350: the forms the image refused cross the PCH by the snapshot: a
/// lambda in a requires-expression ([expr.prim.req]), a local class named
/// by a deduced return type ([dcl.spec.auto]), a requires-expression in a
/// type.
#[test]
fn lambda_forms_cross() {
    let d = scratch("wp1350r");
    for (name, header, tu) in [
        (
            "a",
            "template <class T> concept C = requires { [] {}; };\nstatic_assert(C<int>);\n",
            "static_assert(C<long>);\nint main() {}\n",
        ),
        (
            "b",
            "inline auto lret() { struct R { int q = 11; }; return R{}; }\n",
            "int main() { return lret().q - 11; }\n",
        ),
        (
            "c",
            "template <bool B, class X, class Y> struct cond { using type = X; };
template <class F> using pick = typename cond<requires (F& f) { f(); }, int, long>::type;
inline pick<void(*)()> v = 1;\n",
            "int main() { return v - 1; }\n",
        ),
    ] {
        std::fs::write(d.join(format!("{name}.h")), header).unwrap();
        let (h, pch, cpp) = (format!("{name}.h"), format!("{name}.pch"), format!("{name}.cpp"));
        let o = ok(&d, &["-std=c++26", "-v", "-x", "c++-header", &h, "-o", &pch]);
        assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
        std::fs::write(d.join(&cpp), tu).unwrap();
        same(&d, &pch, &["-std=c++26", "-S", "-emit-llvm"], &cpp);
    }
}

/// Reduced from `cpp.include/pch-body-forms-errors.cpp` under `--pch-diff
/// check` (found by WP-1343, not fixed; pre-existing: the 1.1.0 release
/// candidate reproduced it under the image's `check` with `-fsyntax-only`
/// or `-S -emit-llvm`, at every `-std`): a header's polymorphic class with
/// a derived class, in a unit that never names either, had the base
/// destructor's exception specification evaluated by the replay
/// (`'void () noexcept'`) and still unevaluated from the image
/// ([except.spec]/8). The snapshot agrees with replay.
#[test]
fn virtual_dtor_noexcept_matches_replay() {
    let d = scratch("wp1343v");
    produce(&d, "a", "struct B { virtual ~B() {} };\nstruct D : B { int d = 2; };\n");
    std::fs::write(d.join("a.cpp"), "int main() { return 0; }\n").unwrap();
    let check = ["-fkiln-pch-snapshot=check", "-include-pch", "a.pch", "-fsyntax-only", "a.cpp"];
    assert_eq!(ok(&d, &check).stderr, "");
}

/// WP-1352 (spec/modules.md §11.11 "Forms carried by WP-1352"): an
/// explicit specialization declared, then defined after other
/// declarations, with a base naming a template declared in between, an
/// out-of-line member, a friend and a member named by a later
/// declaration (`char_traits<char8_t>` in libc++'s `<__fwd/string.h>`,
/// [temp.expl.spec]/7).
#[test]
fn declared_then_defined_explicit_specializations_are_carried() {
    let header = "\
namespace n {
template <class T> struct tr;
template <> struct tr<char>;
template <> struct tr<short>;
template <class C, class I, I E> struct base {
  using int_type = I;
  static constexpr int_type eof() { return E; }
  static int size() { return sizeof(C); }
};
}
namespace n {
template <> struct tr<char> : base<char, int, -1> {
  static int twice(int);
  friend int peek(tr) { return 7; }
};
inline int tr<char>::twice(int x) { return 2 * x; }
template <> class tr<short> { public: int v = 4; };
inline int use() { return tr<char>::twice(tr<char>::size()) + tr<short>().v; }
}
";
    let tu = "\
int main() { return n::use() == 6 && n::tr<char>::eof() == -1 && peek(n::tr<char>()) == 7 ? 0 : 1; }
";
    snapshot_agrees("wp1352a", header, tu);
}

/// WP-1352: a partial specialization declared, then defined with a
/// template-head of its own (`tuple_element<I, tuple<T...>>` in
/// libstdc++'s `<bits/stl_pair.h>`, `hash<bitset<N>>` in libc++'s
/// `<bitset>`; [temp.spec.partial]/1): the definition's members name its
/// own parameters, and a redefinition in the unit points at it.
#[test]
fn redeclared_partial_specializations_are_carried() {
    let header = "\
typedef unsigned long sz;
template <class... T> struct tup;
template <sz I, class T> struct elem;
template <sz I, class... Ts> struct elem<I, tup<Ts...>>;
template <class T> struct hsh;
template <sz N> struct bits;
template <sz N> struct hsh<bits<N>>;
template <class A, class B> struct pr {};
template <class A, class B> struct elem<0, pr<A, B>> { using type = A; };
template <class... Ts> struct tup { static constexpr int n = sizeof...(Ts); };
template <sz J, class... Us> struct elem<J, tup<Us...>> { using type = int; static constexpr sz k = J; };
template <sz N> struct bits { int v = N; };
template <sz M> struct hsh<bits<M>> { int operator()(bits<M> b) const { return b.v + M; } };
";
    let tu = "\
int main() {
  elem<1, tup<char, long>>::type x = 2;
  return x + hsh<bits<3>>()(bits<3>()) - 8 + (int)elem<4, tup<>>::k - 4;
}
";
    snapshot_agrees("wp1352b", header, tu);
    let d = scratch("wp1352b-redef");
    let note = produce(&d, "a", header);
    assert!(note.contains("': snapshot written"), "{note}");
    let tu = "template <sz M> struct hsh<bits<M>> {};\nint main() {}\n";
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "a.cpp");
    assert!(o.stderr.contains("a.h:13:24: note: previous definition is here"), "{}", o.stderr);
}

/// WP-1352: implicit specializations of variable templates (`e_v<double>`
/// in libstdc++'s `<numbers>`, [temp.inst]/7): reference records written
/// where the unit made them, so the importer makes them in the same
/// place; their initializers come with the bodies.
#[test]
fn variable_template_specializations_are_carried() {
    let header = "\
namespace n {
template <class T> using fp = T;
template <class T> inline constexpr T e_v = fp<T>(2.5);
template <> inline constexpr float e_v<float> = 1.5f;
template <class T> constexpr T sq_v = e_v<T> * e_v<T>;
inline constexpr double e = e_v<double>;
inline constexpr long sq = sq_v<long>;
static_assert(e_v<int> == 2);
template <class T> T g_v = T(3);
template <class T> constexpr T twice() { return e_v<T> * 2; }
inline int use() { return g_v<int> + int(e) + int(twice<double>()); }
}
";
    let tu = "\
static_assert(n::e == 2.5 && n::sq == 4 && n::e_v<float> == 1.5f && n::e_v<long> == 2);
int main() { return n::use() == 10 && n::sq_v<double> == 6.25 ? 0 : 1; }
";
    snapshot_agrees("wp1352c", header, tu);
}

/// WP-1352: a template named as an explicit argument in an expression a
/// type holds (`decltype(__sfinae_test_impl<_Templ, _Args...>(0))` in
/// libc++'s `<__type_traits/is_valid_expansion.h>`): the type is built
/// after the template's record. The losing candidate's specialization is
/// made where replay makes it too, from its reference record.
#[test]
fn template_arguments_in_type_expressions_are_carried() {
    let header = "\
struct yes { static constexpr bool value = true; };
struct no { static constexpr bool value = false; };
template <template <class...> class T, class... A, class = T<A...>> yes probe(int) { return {}; }
template <template <class...> class, class...> no probe(...) { return {}; }
template <template <class...> class T, class... A>
using valid = decltype(probe<T, A...>(0));
template <class X> using ptr = X *;
inline bool use() { return valid<ptr, int>::value && !valid<ptr, int, int>::value; }
inline yes (*addr())(int) { return &probe<ptr, int>; }
";
    let tu = "\
template <class X> using cst = const X;
int main() { return use() && addr() && valid<cst, int>::value && !valid<cst>::value ? 0 : 1; }
";
    snapshot_agrees("wp1352d", header, tu);
}

/// WP-1352: explicit instantiation declarations of function template
/// specializations (`extern template ostream& __ostream_insert(...)` in
/// libstdc++'s `<bits/ostream_insert.h>`, [temp.explicit]/11): reference
/// records listed as such, so a use in the unit does not instantiate the
/// definition, and an explicit specialization after one is diagnosed as
/// after a replay.
#[test]
fn explicit_function_instantiation_declarations_are_carried() {
    let header = "\
namespace n {
template <class C> struct os { C c; };
template <class C> os<C> &insert(os<C> &o, const C *s, long k) { o.c = s[k]; return o; }
extern template os<char> &insert(os<char> &, const char *, long);
template <class T> T twice(T t) { return t + t; }
extern template int twice<int>(int);
inline int use() { os<char> o{}; return insert(o, \"ab\", 1).c + twice(2); }
}
";
    let tu = "\
template int n::twice<int>(int);
template n::os<char> &n::insert(n::os<char> &, const char *, long);
int main() { return n::use() == 'b' + 4 && n::twice(3L) == 6 ? 0 : 1; }
";
    snapshot_agrees("wp1352e", header, tu);
    let d = scratch("wp1352e-redef");
    let note = produce(&d, "a", header);
    assert!(note.contains("': snapshot written"), "{note}");
    std::fs::write(d.join("b.cpp"), "namespace n { template <> int twice<int>(int); }\n").unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "b.cpp");
    let want = "explicit specialization of 'twice<int>' after instantiation";
    assert!(o.stderr.contains(want), "{}", o.stderr);
}

/// WP-1352: explicit specializations of function templates
/// (`template <> double __do_strtod<double>(...)` in libc++'s `<locale>`,
/// [temp.expl.spec]): the written declaration is a function record listed
/// with its template and converted arguments; the unit's uses of the
/// specialization are marked after the bodies. A redefinition points at
/// the header's.
#[test]
fn explicit_function_specializations_are_carried() {
    let header = "\
namespace n {
template <class T> T conv(const char *s, char **e);
template <> float conv<float>(const char *s, char **e);
template <> inline double conv<double>(const char *s, char **e) { return s[0] - '0' + 0.5; }
template <class T> T pick(T a) { return a; }
template <> inline int pick(int a) { return a + 1; }
template <> constexpr long pick<long>(long a) { return a * 2; }
inline double use() { return conv<double>(\"3\", nullptr) + pick(1) + pick(2L); }
}
";
    let tu = "\
template <> float n::conv<float>(const char *s, char **) { return 1.5f; }
static_assert(n::pick(5L) == 10);
int main() { return n::use() == 3.5 + 2 + 4 && n::conv<float>(\"\", nullptr) == 1.5f && n::pick('a') == 'a' ? 0 : 1; }
";
    snapshot_agrees("wp1352f", header, tu);
    let d = scratch("wp1352f-redef");
    let note = produce(&d, "a", header);
    assert!(note.contains("': snapshot written"), "{note}");
    let redef = "namespace n {\n\
template <> inline double conv<double>(const char *, char **) { return 0; }\n}\n";
    std::fs::write(d.join("b.cpp"), redef).unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "b.cpp");
    assert!(o.stderr.contains("a.h:4:27: note: previous definition is here"), "{}", o.stderr);
}

/// WP-1352: explicit instantiation declarations of single members of a
/// class template specialization (`extern template basic_string<char>::
/// basic_string(const basic_string &)` in libc++'s `<string>`,
/// [temp.explicit]/3): member reference records with the directive's
/// location, listed as such whether the header uses the member or not;
/// a use in the unit does not instantiate the definition, and an explicit
/// specialization after one is diagnosed at the directive.
#[test]
fn explicit_member_instantiation_declarations_are_carried() {
    let header = "\
namespace n {
template <class T> struct S {
  S() : v(0) {}
  S(const S &o) : v(o.v + 1) {}
  int get() const { return v + 1; }
  int other() const { return v + 2; }
  int v;
};
extern template S<int>::S(const S &);
extern template int S<int>::get() const;
extern template int S<int>::other() const;
inline int use(const S<int> &s) { S<int> t(s); return t.get(); }
}
";
    let tu = "\
template n::S<int>::S(const n::S<int> &);
template int n::S<int>::get() const;
template int n::S<int>::other() const;
int main() { n::S<int> s; return n::use(s) + s.other() == 4 ? 0 : 1; }
";
    snapshot_agrees("wp1352g", header, tu);
    let d = scratch("wp1352g-spec");
    let note = produce(&d, "a", header);
    assert!(note.contains("': snapshot written"), "{note}");
    let spec = "namespace n { template <> int S<int>::other() const { return 0; } }\n";
    std::fs::write(d.join("b.cpp"), spec).unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "b.cpp");
    assert!(
        o.stderr.contains("explicit specialization of 'other' after instantiation"),
        "{}",
        o.stderr
    );
    assert!(
        o.stderr.contains("a.h:11:29: note: explicit instantiation first required here"),
        "{}",
        o.stderr
    );
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
    ok(&d, &["-std=c++26", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    let args =
        ["-std=c++26", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch", "-fsyntax-only"];
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
    let args =
        ["-std=c++26", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch", "-fsyntax-only"];
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
    let args =
        ["-std=c++26", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch", "-fsyntax-only"];
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

/// WP-1353: a class template named through a qualifier in a dependent
/// type (`typename pick<...>::template result<...>`, the `_Or` shape of
/// libc++'s `<__iterator/iterator_traits.h>`, [temp.names]/3) is a
/// dependent type record of tag 7 (`QualTst`): the template keyword,
/// the qualifier's steps and the plain specialization it names.
#[test]
fn qualified_template_names_are_carried() {
    let header = "\
struct no { static constexpr bool value = false; };
template <bool> struct pick;
template <> struct pick<true> {
  template <class R, class F, class... Rest>
  using result = typename pick<!bool(F::value) && sizeof...(Rest) != 0>::template result<F, Rest...>;
};
template <> struct pick<false> { template <class R, class...> using result = R; };
template <class... A> using any = typename pick<sizeof...(A) != 0>::template result<no, A...>;
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T *> { static constexpr bool value = true; };
template <class T> struct ptr_or_char : any<is_ptr<T>, is_ptr<T *>> {};
template <class T> concept ptrish = any<is_ptr<T>>::value;
struct holder { template <class T> struct box { T v; }; };
template <class T> struct wrap { typename holder::template box<T> b; };
";
    let tu = "\
static_assert(ptr_or_char<int>::value && ptrish<int *> && !ptrish<int>);
int main() { wrap<int> w{{3}}; return w.b.v - 3; }
";
    snapshot_agrees("wp1353q", header, tu);
}

/// WP-1353: a header's `std::align_val_t` redeclares the enumeration sema
/// declares implicitly for the replaceable allocation functions
/// ([new.delete]/1, [cstddef.syn]) when a `new` came first, in the
/// program's own `std`. The type is an `Implicit` record of kind 10 the
/// reader resolves to the importer's implicit enumeration, and the
/// header's declaration is its redeclaration.
#[test]
fn implicit_align_val_t_redeclarations_are_carried() {
    let header = "\
namespace std { typedef decltype(sizeof 0) size_t; }
inline int *make() { return new int(7); }
namespace std { enum class align_val_t : size_t {}; }
inline std::size_t align_of(std::align_val_t a) { return static_cast<std::size_t>(a); }
";
    let tu = "\
int main() { int *p = make(); int r = *p - 7; delete p; return r + int(align_of(std::align_val_t{2})) - 2; }
";
    snapshot_agrees("wp1353a", header, tu);
}

/// WP-1353: `__attribute__((__deprecated__))` on an explicit instantiation
/// declaration (`__locale`'s `extern template class
/// _LIBCPP_DEPRECATED_... codecvt_byname<...>`): a word attribute in the
/// instantiation's attribute list.
#[test]
fn deprecated_explicit_instantiations_are_carried() {
    let header = "\
template <class T> struct old { T v; int get() const { return int(v); } };
extern template struct __attribute__((__deprecated__)) old<char>;
";
    let tu = "int main() { old<int> o{2}; return o.get() - 2; }\n";
    snapshot_agrees("wp1353d", header, tu);
}

/// WP-1353: non-dependent using-declarations of base members in a class
/// ([namespace.udecl]/3): a member function, a data member and a typedef
/// of an ordinary base, with access, and a member function template of a
/// class template specialization base (`using _RangeAdaptor<_Drop>::
/// operator();` in libstdc++'s `<ranges>`), whose shadow names a member
/// reference record.
#[test]
fn class_using_declarations_are_carried() {
    let header = "\
struct A { int f(int) const { return 1; } int v = 5; typedef long L; };
struct B : A { int f(long) const { return 2; } using A::f; using A::v; using A::L; };
class C : A { public: using A::f; private: using A::v; };
template <class D> struct adaptor {
  template <class X> constexpr int operator()(X x) const { return static_cast<const D &>(*this)(x, 1); }
};
struct drop;
struct drop : adaptor<drop> {
  constexpr int operator()(int a, int b) const { return a + b; }
  using adaptor<drop>::operator();
};
inline constexpr drop dr;
";
    let tu = "\
static_assert(dr(2) == 3);
int main() { B b; C c; B::L l = 3; return b.f(0) + b.f(0L) + b.v + c.f(0) + int(l) + dr(1, 2) - 15; }
";
    snapshot_agrees("wp1353u", header, tu);
    let d = scratch_of("wp1353u");
    std::fs::write(d.join("b.cpp"), "int g(C c) { return c.v; }\n").unwrap();
    let args =
        ["-std=c++20", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch", "-fsyntax-only"];
    let o = kiln(&d, &[&args[..], &["b.cpp"]].concat());
    assert!(o.stderr.contains("'v' is a private member of 'C'"), "{}", o.stderr);
    assert!(!o.stderr.contains("differs"), "{}", o.stderr);
}

/// WP-1353: an explicit specialization of a member class template
/// defined outside its class (`template <> struct
/// __future_base::_Result<void>` in libstdc++'s `<future>`,
/// [temp.expl.spec]/2), beside an out-of-class partial specialization.
#[test]
fn out_of_class_member_explicit_specializations_are_carried() {
    let header = "\
struct outer {
  struct base { virtual ~base() {} virtual int k() const { return 0; } };
  template <class R> struct res : base { R v{}; int k() const override { return 1; } };
};
template <class R> struct outer::res<R &> : outer::base { R *p = nullptr; int k() const override { return 2; } };
template <> struct outer::res<void> : outer::base {
  typedef void result_type;
  int k() const override { return 3; }
private:
  void destroy() { delete this; }
};
";
    let tu = "\
int main() { outer::res<int> a; outer::res<int &> b; outer::res<void> c; return a.k() + b.k() + c.k() - 6; }
";
    snapshot_agrees("wp1353f", header, tu);
}

/// WP-1353: special members of a class that is not a template defaulted
/// outside the class ([dcl.fct.def.default]/5): user-provided, defined
/// where written; the importer defaults its redeclaration again once the
/// bodies (the in-class initializers) are built.
#[test]
fn out_of_line_defaulted_members_are_carried() {
    let header = "\
struct M { int x = 2; constexpr M() = default; M(M &&) noexcept; M &operator=(M &&) noexcept; ~M(); };
inline M::M(M &&) noexcept = default;
inline M &M::operator=(M &&) noexcept = default;
inline M::~M() = default;
struct P { constexpr P(); int y; M m; };
constexpr P::P() = default;
inline int use() { P p; M a; M b(static_cast<M &&>(a)); b = static_cast<M &&>(a); return p.m.x + b.x; }
";
    let tu = "int main() { M c; M d(static_cast<M &&>(c)); return use() + d.x - 6; }\n";
    snapshot_agrees("wp1353o", header, tu);
}

/// WP-1353: a call in a template definition of a function with an
/// argument-dependent `diagnose_if` attribute and non-dependent
/// arguments is checked when the unit instantiates the template, after
/// the image as after a replay (the importer fills the recheck map from
/// the bodies it built).
#[test]
fn diagnose_if_rechecks_follow_the_image() {
    let d = scratch("wp1353i");
    let header = "\
inline int pos(int x) __attribute__((diagnose_if(x < 0, \"negative\", \"error\"))) { return x; }
template <class T> int call() { return pos(-1) + int(sizeof(T)); }
";
    let note = produce(&d, "a", header);
    assert!(note.contains("': snapshot written"), "{note}");
    std::fs::write(d.join("a.cpp"), "int main() { return call<char>(); }\n").unwrap();
    let o = same(&d, "a.pch", &["-fsyntax-only"], "a.cpp");
    assert!(o.stderr.contains("a.h:2:46: error: negative"), "{}", o.stderr);
}

/// WP-1358 (spec/modules.md §11.11 "Forms carried by WP-1358"):
/// complete specializations of class templates with constrained partial
/// specializations ([temp.spec.partial.match]/2; `__atomic_base` in
/// libc++'s `<__atomic/aliases.h>`), named directly or completed as a
/// base: the image records the partial specialization each was
/// instantiated from, as the importer reads them before the constraints.
#[test]
fn constrained_partial_specialization_selections_are_carried() {
    let header = "\
template <class T> concept I = sizeof(T) == 4;
template <class T> concept J = sizeof(T) == 8;
template <class T> struct A { int a; static constexpr int k = 0; int get() const { return a; } };
template <I T> struct A<T> { int b; static constexpr int k = 1; int get() const { return b + 10; } };
template <class T> requires J<T> struct A<T*> { long c; static constexpr int k = 2; };
template <class T> struct A<T*> { int d; static constexpr int k = 3; };
using AI = A<int>;
inline int f() { A<int> x{}; A<char> y{}; return x.get() + y.get() + A<double*>::k + A<char*>::k; }
template <class T> concept Big = sizeof(T) > 2;
template <class T> struct B { int a; char b[3]; };
template <Big T> struct B<T> { int c; };
template <class T> struct X : B<T> {};
inline int g() { X<char> x; X<int> y; return sizeof(x) + sizeof(y); }
";
    let tu = "\
static_assert(A<double*>::k == 2 && A<char*>::k == 3 && AI::k == 1 && A<char>::k == 0);
static_assert(sizeof(X<char>) == 8 && sizeof(X<long>) == 4);
int main() { return f() == 15 && g() == 12 ? 0 : 1; }
";
    snapshot_agrees("wp1358a", header, tu);
}

/// WP-1358: a specialization a declaration's record names that `prescan`
/// does not reach (the implicit default constructor of `ptag<1>` in a
/// `decltype`, libc++'s `<__compare/compare_partial_order_fallback.h>`)
/// is written before the record, which is written again.
#[test]
fn specializations_named_while_recorded_are_carried() {
    let header = "\
template <unsigned N> struct ptag : ptag<N - 1> {};
template <> struct ptag<0> {};
namespace n {
struct fn {
  template <class T> static constexpr auto go(T t, ptag<1>) -> decltype(t + 1) { return t + 1; }
  template <class T> static constexpr auto go(T t, ptag<0>) -> decltype(t) { return t; }
  template <class T>
  constexpr auto operator()(T t) const noexcept(noexcept(go(t, ptag<1>()))) -> decltype(go(t, ptag<1>())) {
    return go(t, ptag<1>());
  }
};
inline constexpr auto f = fn{};
}
";
    let tu = "\
static_assert(n::f(1) == 2);
int main() { return n::f(2) == 3 ? 0 : 1; }
";
    snapshot_agrees("wp1358b", header, tu);
}

/// WP-1358: specializations of member function templates of class
/// specializations (`chrono::hours(__h)` in libc++'s
/// `<__chrono/duration.h>`, [temp.mem]/1): named through the member
/// template's member reference, a losing candidate's included.
#[test]
fn member_template_specializations_are_carried() {
    let header = "\
template <class R, int P = 1>
struct dur {
  R r;
  template <class R2>
  constexpr explicit dur(const R2& x) : r(static_cast<R>(x) * P) {}
  template <class R2, int Q>
  constexpr dur(const dur<R2, Q>& d) : r(d.r * Q / P) {}
  constexpr R count() const { return r; }
  template <class U> U as() const { return U(r); }
};
using hours = dur<long, 60>;
using mins = dur<long>;
constexpr hours make_h(unsigned long long h) { return hours(static_cast<long>(h)); }
inline long in_mins(hours h) { return mins(h).count() + h.as<int>(); }
";
    let tu = "\
static_assert(make_h(2).count() == 120);
int main() { return in_mins(make_h(1)) == 3660 && make_h(1).as<long>() == 60 ? 0 : 1; }
";
    snapshot_agrees("wp1358c", header, tu);
}

/// WP-1358: a class specialization first named in a body
/// (`dur<long>(h)`) is written where the walk meets its member
/// template's specializations, a losing candidate's included.
#[test]
fn member_template_specializations_of_body_named_classes_are_carried() {
    let header = "\
template <class R, int P = 1> struct dur {
  R r;
  template <class R2> constexpr explicit dur(const R2 &x) : r(static_cast<R>(x) * P) {}
  template <class R2, int Q> constexpr dur(const dur<R2, Q> &d) : r(d.r * Q / P) {}
  constexpr R count() const { return r; }
};
using hours = dur<long, 60>;
inline long in_mins(hours h) { return dur<long>(h).count(); }
";
    snapshot_agrees(
        "wp1358j",
        header,
        "int main() { return in_mins(hours(2)) == 7200 ? 0 : 1; }\n",
    );
}

/// WP-1358: specializations of a partial specialization declared before
/// its definition (`iterator_traits<_Tp*>` in libstdc++'s
/// `<bits/iterator_concepts.h>`) or named by the next declaration, whose
/// record the definition's end then precedes.
#[test]
fn specializations_of_partial_specializations_are_carried() {
    let header = "\
template <class I> struct traits;
template <class T> struct traits<T*>;
template <class I> struct traits { using value = int; };
template <class T> struct traits<T*> { using value = T; static constexpr int k = 2; };
using cp = traits<char*>::value;
inline int kk() { return traits<long*>::k + sizeof(cp); }
template <class T> struct tr2 {};
template <class T> struct tr2<T&> { static constexpr int k = 5; };
inline constexpr int k2 = tr2<int&>::k;
";
    let tu = "int main() { return kk() == 3 && k2 == 5 && traits<int*>::k == 2 ? 0 : 1; }\n";
    snapshot_agrees("wp1358d", header, tu);
}

/// WP-1358: constrained variable templates and ones with (constrained)
/// partial specializations (`<numbers>`'s `e_v`): the image records
/// which partial specialization each specialization took.
#[test]
fn variable_template_partial_selections_are_carried() {
    let header = "\
template <class T> concept fp = __is_same(T, float) || __is_same(T, double);
template <class T> concept big = sizeof(T) >= 8;
template <class T> struct ill { static_assert(sizeof(T) == 0, \"bad\"); };
template <class T> inline constexpr T e_v = ill<T>{};
template <fp T> inline constexpr T e_v<T> = T(2.5);
template <class T> inline constexpr int k_v = 1;
template <class T> inline constexpr int k_v<T*> = 2;
template <class T> requires big<T> inline constexpr int k_v<T*> = 3;
template <big T> inline constexpr int c_v = sizeof(T);
inline constexpr double e = e_v<double>;
inline constexpr float ef = e_v<float>;
inline constexpr int ks = k_v<char*> * 100 + k_v<long*> * 10 + k_v<int> + c_v<long>;
";
    let tu = "\
static_assert(e == 2.5 && ef == 2.5f && ks == 239 && k_v<double*> == 3 && e_v<double> == 2.5);
int main() { return ks == 239 ? 0 : 1; }
";
    snapshot_agrees("wp1358e", header, tu);
}

/// WP-1358: dependent expressions as partial specialization arguments
/// (`sizeof(T) * 0`, the `<segmented_iterator.h>` detection idiom).
#[test]
fn partial_specialization_argument_expressions_are_carried() {
    let header = "\
typedef unsigned long size_t;
struct ft { static constexpr bool value = false; };
struct tt { static constexpr bool value = true; };
template <class T> struct seg;
template <> struct seg<int> { using type = int; };
template <class T, size_t = 0> struct has : ft {};
template <class T> struct has<T, sizeof(T) * 0> : tt {};
template <class T> using is_seg = has<seg<T>>;
template <int N, int M = N % 2> struct par { static constexpr int k = 0; };
template <int N> struct par<N, (N * 0)> { static constexpr int k = 1; };
inline bool f() { return is_seg<int>::value && !is_seg<long>::value && par<3>::k == 0; }
";
    let tu = "\
static_assert(is_seg<int>::value && !is_seg<char>::value);
int main() { return f() && par<4>::k == 1 && par<5>::k == 0 ? 0 : 1; }
";
    snapshot_agrees("wp1358f", header, tu);
}

/// WP-1358: value and pack explicit template arguments of a name in a
/// body (`<chrono>`'s literal operators, `operator""_s<'3', ...>`).
#[test]
fn value_and_pack_template_arguments_in_bodies_are_carried() {
    let header = "\
template <char... C> struct digits { static constexpr long v = 0; };
template <char C, char... R> struct digits<C, R...> {
  static constexpr long p() { long x = 1; for (unsigned i = 0; i < sizeof...(R); ++i) x *= 10; return x; }
  static constexpr long v = (C - '0') * p() + digits<R...>::v;
};
struct sec { long n; };
template <char... C> constexpr sec operator\"\"_s() { return sec{digits<C...>::v}; }
template <int N, char K> constexpr int pick() { return N + K; }
inline long epoch() { return (378691210_s).n + pick<3, 'a'>(); }
";
    let tu = "\
static_assert((12_s).n == 12);
int main() { return epoch() == 378691210 + 100 ? 0 : 1; }
";
    snapshot_agrees("wp1358g", header, tu);
}

/// WP-1358: a completed specialization whose member function has a
/// trailing requires-clause, or whose member template has constraints,
/// gets them back after the image's constraints are in; overloads
/// differing only in constraints, and a constrained member template's
/// specializations the header makes (which the image refused), cross by
/// the snapshot ([temp.constr.decl]).
#[test]
fn constrained_members_of_completed_specializations() {
    let header = "\
template <class T> concept I = sizeof(T) == 4;
template <class T> struct W { int f() const requires I<T> { return 1; } int g() const { return 0; } };
inline int use() { return W<int>{}.f() + W<char>{}.g(); }
";
    snapshot_agrees("wp1358h", header, "int main() { return use() == 1 ? 0 : 1; }\n");
    // A constrained member template gets its constraints back (it was
    // declared without them, and its specializations in the consumer
    // mangled without `Tk`).
    let header = "\
template <class T> concept I = sizeof(T) == 4;
template <class T> struct W { template <I U> W(U, int) : k(2) {} W() : k(0) {} template <class U> requires I<U> int h(U) const { return 100; } int k; int g() const { return k + 1; } };
inline int use() { W<char> w; return w.g(); }
";
    let tu =
        "int main() { W<char> w(1, 0); return use() == 1 && w.h(2) == 100 && w.k == 2 ? 0 : 1; }\n";
    snapshot_agrees("wp1358k", header, tu);
    let d = scratch("wp1358i");
    for (n, h) in [
        "template <class T> struct A {\n  int f() requires (sizeof(T) > 1) { return 1; }\n  \
         int f() const { return 2; }\n};\ninline int g() { return sizeof(A<long>); }\n",
        "template <class T> concept I = sizeof(T) == 4;\ntemplate <class T> struct W {\n  \
         template <class U> requires I<U> int g(U) const { return 10; }\n  \
         template <class U> requires (!I<U>) int g(U) const { return 20; }\n  \
         int k() const { return 1; }\n};\ninline int use() { return W<int>{}.k(); }\n",
    ]
    .into_iter()
    .enumerate()
    {
        let (hf, pch) = (format!("h{n}.h"), format!("h{n}.pch"));
        std::fs::write(d.join(&hf), h).unwrap();
        let o = ok(&d, &["-std=c++20", "-v", "-x", "c++-header", &hf, "-o", &pch]);
        assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
        let cpp = format!("h{n}.cpp");
        std::fs::write(d.join(&cpp), "int main() { return use() == 1 ? 0 : 1; }\n").unwrap();
        same(&d, &pch, &["-std=c++20", "-S", "-emit-llvm"], &cpp);
    }
    // A specialization of a constrained member template the header
    // makes: its mangled name has the instantiated head's constraints.
    std::fs::write(d.join("c.h"), "template <class T> concept I = sizeof(T) == 4;\ntemplate <class T> struct W { template <I U> static int h(U) { return 100; } int k; };\ninline int use() { W<char> w{}; return W<int>::h(1) + w.k; }\n").unwrap();
    let o = ok(&d, &["-std=c++20", "-v", "-x", "c++-header", "c.h", "-o", "c.pch"]);
    assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
    std::fs::write(d.join("c.cpp"), "int main() { return use() == 100 ? 0 : 1; }\n").unwrap();
    same(&d, "c.pch", &["-std=c++20", "-S", "-emit-llvm"], "c.cpp");
}

/// WP-1358 found (pre-existing, 1.1.0): a dependent concept-id in a
/// member typedef of a specialization the importer completes while it
/// reads the records is satisfied before the concept's constraint is
/// patched in: the image's `X<char>::t` was `int`. The snapshot agrees
/// with replay ([temp.names]/9).
#[test]
fn record_time_completion_concept_id_matches_replay() {
    let d = scratch("wp1358x");
    let header = "\
template <class T> concept Big = sizeof(T) > 2;
template <bool B, class T, class F> struct cond { using type = F; };
template <class T, class F> struct cond<true, T, F> { using type = T; };
template <class T> struct X { using t = typename cond<Big<T>, int, char>::type; t m; };
inline int f() { X<char> x; return sizeof(x); }
";
    std::fs::write(d.join("a.h"), header).unwrap();
    ok(&d, &["-std=c++20", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    std::fs::write(d.join("a.cpp"), "static_assert(sizeof(X<char>) == 1);\nint main() {}\n")
        .unwrap();
    let check = [
        "-std=c++20",
        "-fkiln-pch-snapshot=check",
        "-include-pch",
        "a.pch",
        "-fsyntax-only",
        "a.cpp",
    ];
    assert_eq!(ok(&d, &check).stderr, "");
}

/// WP-1358 found (pre-existing, 1.1.0): a variable template's value
/// needed by a record-time completion (`cond<big_v<T>, ...>`) is
/// evaluated before the initializers came with the bodies, through the
/// image. The snapshot agrees with replay.
#[test]
fn record_time_completion_variable_template_matches_replay() {
    let d = scratch("wp1358y");
    let header = "\
template <class T> constexpr bool big_v = sizeof(T) > 2;
template <bool B, class T, class F> struct cond { using type = F; };
template <class T, class F> struct cond<true, T, F> { using type = T; };
template <class T> struct X { using t = typename cond<big_v<T>, int, char>::type; t m; };
inline int g() { X<int> x; return sizeof(x); }
";
    std::fs::write(d.join("a.h"), header).unwrap();
    ok(&d, &["-std=c++20", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    std::fs::write(d.join("a.cpp"), "static_assert(sizeof(X<int>) == 4);\nint main() {}\n")
        .unwrap();
    let check = [
        "-std=c++20",
        "-fkiln-pch-snapshot=check",
        "-include-pch",
        "a.pch",
        "-fsyntax-only",
        "a.cpp",
    ];
    assert_eq!(ok(&d, &check).stderr, "");
}

/// WP-1360: a `noexcept` operand naming a member its class declares
/// later (a complete-class context, [class.mem.general]/7) goes through
/// the late `noexcept` path, in a class template and a plain class.
#[test]
fn noexcept_naming_a_later_member_is_carried() {
    let header = "\
template <class T> struct box { void swap(box &o) noexcept(noexcept(o.get())) { v = o.v; } T get() const noexcept { return v; } T v; };
struct plain { int f() noexcept(noexcept(g())) { return g(); } int g() const { return 3; } };
inline int use() { box<int> a{1}, b{2}; a.swap(b); return plain{}.f() + a.v - 2; }
";
    let tu = "static_assert(noexcept(box<int>{}.swap(*(box<int> *)0)));\n\
static_assert(!noexcept(plain{}.f()));\n\
int main() { return use() - 3; }\n";
    snapshot_agrees("wp1360n", header, tu);
}

/// WP-1360: a class whose base names the class itself (`struct D :
/// B<D>`, the curiously recurring template pattern): its bases are
/// written after its record (`BASES_SELF`).
#[test]
fn self_naming_bases_are_carried() {
    let header = "\
template <class D> struct base { int twice() const { return 2 * static_cast<const D *>(this)->v(); } };
template <class T> struct tag {};
struct derived : base<derived> { int v() const { return 2; } };
struct both : base<both>, tag<both *> { int v() const { return 1; } };
";
    let tu = "int main() { derived d; both b; base<derived> *p = &d; (void)p; return d.twice() + b.twice() - 6; }\n";
    snapshot_agrees("wp1360c", header, tu);
}

/// WP-1360: an implicit special member of a closed class named in a
/// later declaration's type (`noexcept(C())`, `decltype(C()(...))`) is
/// recorded where first named.
#[test]
fn implicit_member_named_in_a_type_is_carried() {
    let header = "\
struct cmp { bool operator()(int a, int b) const { return a < b; } };
struct fn { template <class T> static auto go(T a, T b) noexcept(noexcept(cmp()(a, b))) -> decltype(cmp()(a, b)) { return cmp()(a, b); } };
";
    let tu =
        "static_assert(!noexcept(fn::go(1, 2)));\nint main() { return fn::go(1, 2) ? 0 : 1; }\n";
    snapshot_agrees("wp1360i", header, tu);
}

/// WP-1360: a concept checked in the header (its satisfied
/// requires-expression substituted), a member named in an unevaluated
/// operand of a type (`sizeof(m)`, [expr.prim.id.general]/4), and a
/// function template with a deduced return type declared before its
/// definition and used in between.
#[test]
fn checked_concepts_sizeof_members_and_auto_redecls_are_carried() {
    let header = "\
template <class T> concept testable = requires(T t) { t + 1; };
static_assert(testable<int>);
template <int N> struct raw { char b[N]; };
struct holder { long p; raw<sizeof(p)> r; };
template <class T> auto conv_to(T t);
template <class T> int via(T t) { return int(conv_to(t)); }
template <class T> auto conv_to(T t) { return t + 1; }
";
    let tu = "static_assert(testable<long>);\nstatic_assert(sizeof(holder{}.r) == sizeof(long));\n\
int main() { return via(2) - 3; }\n";
    snapshot_agrees("wp1360r", header, tu);
}

/// WP-1360: a member function template of a class template
/// specialization named in a dependent call (`aux<false>::go(f, l)`): the
/// lookup finds the specialization's member record, whose pattern is the
/// member template.
#[test]
fn member_templates_of_specializations_in_dependent_calls_are_carried() {
    let header = "\
template <bool> struct aux { template <class I> static constexpr int go(I f, I l) { return int(l - f); } };
template <> struct aux<true> { template <class I> static int go(I, I) { return -1; } };
template <class I> constexpr int span(I f, I l) {
  if (__builtin_is_constant_evaluated()) return aux<false>::go(f, l);
  return aux<sizeof(I) == 1>::go(f, l);
}
";
    let tu = "constexpr int a[3] = {};\nstatic_assert(span(a, a + 3) == 3);\n\
int main() { return span(a, a + 2) - 2; }\n";
    snapshot_agrees("wp1360m", header, tu);
}

/// The overloaded member accesses of WP-1368's tests: implicit `this`
/// (a plain overload and member templates), `.` and `->` on a
/// non-dependent object with a static candidate, explicit template
/// arguments, a member hidden by the derived class's, a using-declaration
/// that brings the base's in, a class template's `this->f`, and a private
/// candidate.
const OVL_MEMBERS: &str = "\
struct vec { int a[4]; constexpr const int *begin() const { return a; } constexpr const int *end() const { return a + 4; } };
struct dist_fn {
  constexpr long operator()(const int *f, const int *l, int) const { return l - f + 100; }
  template <class I> constexpr long operator()(I f, I l) const { return l - f; }
  template <class R> constexpr long operator()(R &&r) const { return operator()(r.begin(), r.end()); }
  template <class R> constexpr long twice(R &&r) const { return this->operator()(r) * 2; }
};
struct obj {
  constexpr int f(int) const { return 1; }
  constexpr int f(long) const { return 2; }
  static constexpr int f(double) { return 3; }
  template <class T> constexpr int g(T) const { return 10; }
  constexpr int g(int) const { return 20; }
};
template <class T> constexpr int dot(T t) { obj o; return o.f(t); }
template <class T> constexpr int arrow(T t) { obj o; const obj *p = &o; return p->f(t); }
template <class T> constexpr int targs(T t) { obj o; return o.g<int>(t) + o.g(t); }
struct base { constexpr int h(int) const { return 1; } constexpr int h(char) const { return 4; } };
struct hides : base { constexpr int h(long) const { return 2; } constexpr int h(const char *) const { return 5; } };
struct uses : base { using base::h; constexpr int h(long) const { return 3; } constexpr int h(double) const { return 6; } };
template <class T> constexpr int hid(T t) { hides d; return d.h(t); }
template <class T> constexpr int used(T t) { uses d; return d.h(t); }
template <class V> struct gen {
  V v;
  template <class I> constexpr V run(I i) { return this->run(i, v); }
  template <class I> constexpr V run(I i, V w) { return i + w; }
  constexpr V run(int *, V) { return -1; }
};
struct priv {
  constexpr int p(long) const { return 1; }
private:
  constexpr int p(int) const { return 2; }
};
template <class T> constexpr int callp(T t) { priv x; return x.p(t); }
";

/// WP-1368: an overloaded member access in a dependent call carries its
/// whole candidate set (WP-1360 refused it; before that, the image kept
/// one candidate and every instantiation failed). Each instantiation
/// picks the overload replay picks: the plain function or the template,
/// the static member, the derived class's hiding member, the base's
/// brought in by `using`, the explicit template argument's.
#[test]
fn overloaded_member_accesses_are_carried() {
    let tu = "\
static_assert(dist_fn{}(vec{}) == 4 && dist_fn{}.twice(vec{}) == 8);
static_assert(dot(1) == 1 && dot(1L) == 2 && dot(1.0) == 3);
static_assert(arrow(1) == 1 && arrow(1L) == 2 && arrow(1.0) == 3);
static_assert(targs(1) == 30 && targs(1L) == 20);
static_assert(hid(1) == 2 && hid(\"x\") == 5);
static_assert(used(1) == 1 && used('c') == 4 && used(1L) == 3);
static_assert(gen<int>{5}.run(2) == 7 && callp(1L) == 1);
int main() {
  vec v{};
  gen<long> g{1};
  return int(dist_fn{}(v) + dist_fn{}.twice(v)) - 12 + dot(1L) - 2 + used(1.5) - 6 + int(g.run(1)) - 2;
}
";
    snapshot_agrees("wp1368a", OVL_MEMBERS, tu);
}

/// WP-1368: through the image, access checking still sees a private
/// candidate ([class.access]/4: the better match is chosen, then checked)
/// and an ambiguity among the overloads a using-declaration joins is still one.
#[test]
fn overloaded_member_access_errors_match_replay() {
    let d = scratch("wp1368e");
    std::fs::write(d.join("a.h"), OVL_MEMBERS).unwrap();
    let o = ok(&d, &["-std=c++20", "-v", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    assert!(o.stderr.contains("': snapshot written"), "{}", o.stderr);
    std::fs::write(d.join("a.cpp"), "int a = callp(1);\nint b = used(1u);\n").unwrap();
    let o = same(&d, "a.pch", &["-std=c++20", "-fsyntax-only"], "a.cpp");
    assert_ne!(o.code, 0);
    assert!(o.stderr.contains("'p' is a private member of 'priv'"), "{}", o.stderr);
    assert!(o.stderr.contains("call to member function 'h' is ambiguous"), "{}", o.stderr);
    let check = ["-std=c++20", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
    let o = kiln(&d, &[&check[..], &["-fsyntax-only", "a.cpp"]].concat());
    assert!(!o.stderr.contains("differs"), "{}", o.stderr);
}

/// WP-1368 (a pre-existing WP-1352 validator bug): a partial
/// specialization declared in one block of a namespace and defined in a
/// reopened block (libstdc++'s `iterator_traits<_Tp*>`). Its head's
/// parameters are in the second block, which the validator rejected as
/// "an internal image inconsistency" (no census reason; the header was
/// replayed).
#[test]
fn partial_specialization_defined_in_a_reopened_namespace_is_carried() {
    let header = "\
namespace n { template <class T> struct it; template <class T> struct it<T *>; }
namespace n { template <class T> struct it<T *> { using type = T; static constexpr int k = sizeof(T); }; }
";
    let tu = "static_assert(n::it<long *>::k == sizeof(long));\n\
int main() { n::it<int *>::type x = 0; return x; }\n";
    snapshot_agrees("wp1368n", header, tu);
}

/// WP-1368 (a pre-existing WP-1314 validator bug): an explicit
/// specialization declared in one block of a nested namespace and
/// defined in a reopened one (libc++'s `std { inline namespace __1 {`
/// blocks). The inner namespace's reopening record, written while the
/// specialization's record is open, has the reopened outer block as its
/// lexical context; the validator accepted only none and rejected the
/// image as "an internal image inconsistency" (`<compare>`, `<utility>`,
/// `<tuple>`, `<typeindex>`).
#[test]
fn specialization_defined_in_a_reopened_nested_namespace_is_carried() {
    let header = "\
namespace s { inline namespace v { template <class T> struct tr; template <> struct tr<char>; } }
namespace s { inline namespace v { template <> struct tr<char> { using type = char; static constexpr int k = 1; static int f() { return 2; } }; } }
";
    let tu = "static_assert(s::tr<char>::k == 1);\n\
int main() { s::tr<char>::type c = 0; return c + s::tr<char>::f() - 2; }\n";
    snapshot_agrees("wp1368s", header, tu);
}

/// WP-1369, spec/modules.md §11.11 "Forms carried by WP-1369": functions
/// declared through a typedef of function type ([dcl.fct]/13), at
/// namespace scope and as members, through an alias template, `decltype`
/// and `__typeof`: a later declaration adding a default argument, a
/// `noexcept` in the typedef, out-of-line and later definitions, and a
/// class template's member, as after replay.
#[test]
fn functions_declared_through_a_typedef_are_carried() {
    let header = "\
typedef int F(int);
typedef int N(int) noexcept;
F f;
int f(int = 7);
N nf;
template <class T> using Un = T(T);
template <class T> struct Box { F get; Un<T> twice; static N sget; };
template <class T> int Box<T>::get(int x) { return x + 1; }
template <class T> T Box<T>::twice(T t) { return t + t; }
template <class T> int Box<T>::sget(int x) noexcept { return x * 3; }
struct Q { F mf; F vf; };
inline int Q::mf(int a) { return a - 1; }
inline int use() { return f() + Box<int>{}.get(1); }
int k(int);
decltype(k) dk;
__typeof(k) tk;
Un<long> ul;
";
    let tu = "\
int f(int v) { return v; }
int nf(int v) noexcept { return v + 2; }
int Q::vf(int a) { return a * 4; }
int dk(int a) { return a; }
int tk(int a) { return a + 1; }
long ul(long a) { return a; }
static_assert(noexcept(nf(1)) && !noexcept(f(1)) && noexcept(Box<int>::sget(1)));
int main() {
  Q q; Box<long> b;
  return f() + nf(1) + q.mf(2) + q.vf(1) + b.get(1) + int(b.twice(3)) + Box<int>::sget(1)
       + use() + dk(0) + tk(0) + int(ul(1)) - 293;
}
";
    snapshot_agrees("wp1369t", header, tu);
}

/// WP-1369: the predefined SVE typedefs (`__SVFloat32_t`, `__SVBool_t`,
/// the tuple types) named by a header are the importer's own on
/// `AArch64`; an x86-64 image of the same header names none (they are not
/// declared there). Both agree with replay under `check`.
#[test]
fn predefined_sve_typedefs_are_carried() {
    let header = "\
#ifdef __aarch64__
typedef __SVFloat32_t sv_f32;
typedef __SVBool_t sv_b;
typedef __clang_svint8x2_t sv_i8x2;
sv_f32 vmul(sv_f32, sv_b);
inline int sz() { return sizeof(sv_b *) + sizeof(sv_i8x2 *); }
#else
inline int sz() { return 16; }
#endif
";
    let tu = "int main() { return sz() - 16; }\n";
    let d = scratch("wp1369s");
    std::fs::write(d.join("a.h"), header).unwrap();
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    for target in ["--target=aarch64-linux-gnu", "--target=x86_64-linux-gnu"] {
        let args = [target, "-std=c++20", "-v"];
        let o = ok(&d, &[&args[..], &["-x", "c++-header", "a.h", "-o", "a.pch"]].concat());
        assert!(o.stderr.contains("': snapshot written"), "{target}: {}", o.stderr);
        same(&d, "a.pch", &[target, "-std=c++20", "-S", "-emit-llvm", "-O0"], "a.cpp");
        for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm"]] {
            let check =
                [target, "-std=c++20", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
            let o = ok(&d, &[&check[..], mode, &["a.cpp", "-o", "-"]].concat());
            assert_eq!(o.stderr, "", "{target} {mode:?}");
        }
    }
}

/// WP-1369: the class `__builtin_va_list` names (`std::__va_list` on
/// `AArch64`, copied when a `va_list` is passed by value, with its
/// implicit copy constructor; `__va_list_tag` on x86-64, the decayed
/// parameter type) is the importer's own.
#[test]
fn va_list_class_is_carried() {
    let header = "\
int vsn(const char *, __builtin_va_list);
inline int pass(const char *f, __builtin_va_list v) { return vsn(f, v); }
typedef __builtin_va_list VL;
int take(VL);
";
    let tu = "int main() { return 0; }\n";
    let d = scratch("wp1369v");
    std::fs::write(d.join("a.h"), header).unwrap();
    std::fs::write(d.join("a.cpp"), tu).unwrap();
    for target in ["--target=aarch64-linux-gnu", "--target=x86_64-linux-gnu"] {
        let args = [target, "-std=c++20", "-v"];
        let o = ok(&d, &[&args[..], &["-x", "c++-header", "a.h", "-o", "a.pch"]].concat());
        assert!(o.stderr.contains("': snapshot written"), "{target}: {}", o.stderr);
        same(&d, "a.pch", &[target, "-std=c++20", "-S", "-emit-llvm", "-O0"], "a.cpp");
        let check = [target, "-std=c++20", "-fkiln-pch-snapshot=check", "-include-pch", "a.pch"];
        let o = ok(&d, &[&check[..], &["-fsyntax-only", "a.cpp"]].concat());
        assert_eq!(o.stderr, "", "{target}");
    }
}

/// WP-1369: a member of a base of an implicit instantiation named through
/// a derived-to-base path (`a.fetch_sub(u)`, `atomic<long>` deriving
/// from its base template), and a member class of an implicit
/// instantiation named in a type (`storage<8, 8>::type`): the importer's
/// own instantiation has the base and the member class.
#[test]
fn bases_and_member_classes_of_specializations_are_carried() {
    let header = "\
template <class T> struct base { T v; T fetch_sub(T d) { return v -= d; } };
template <class T> struct atom : base<T> {};
struct latch { atom<long> a; long cd(long u) { return a.fetch_sub(u); } long get() { return a.v; } };
template <unsigned L, unsigned A> struct storage { struct type { alignas(A) unsigned char data[L]; }; };
struct any_ { void *p; storage<sizeof(p), alignof(void *)>::type buf; };
inline int an() { any_ a{}; return sizeof(a.buf); }
";
    let tu = "\
int main() { latch l{}; l.a.v = 5; l.cd(2); return int(l.get()) + an() - 3 - int(sizeof(void *)); }
";
    snapshot_agrees("wp1369b", header, tu);
}

/// WP-1369 found (pre-existing, 1.1.0) and fixed: a specialization the
/// importer completes while it reads the records instantiates its
/// pattern's `alignas(A)` before the attributes' expressions are in (they
/// come with the bodies); the instantiation is a placeholder substituted
/// once they are (`bmi_image/alignfix.rs`), so `U` is 16 bytes through the
/// image as after replay, and `check` finds no difference.
#[test]
fn record_time_completion_alignas_matches_replay() {
    let d = scratch("wp1369x");
    let header = "\
template <unsigned L, unsigned A> struct st { alignas(A) unsigned char data[L]; };
struct U { void *p; st<sizeof(p), alignof(void *)> buf; };
";
    std::fs::write(d.join("a.h"), header).unwrap();
    ok(&d, &["-std=c++20", "-x", "c++-header", "a.h", "-o", "a.pch"]);
    std::fs::write(
        d.join("a.cpp"),
        "int main() { U u{}; return int(sizeof(u)) - 2 * int(sizeof(void *)); }\n",
    )
    .unwrap();
    let check = [
        "-std=c++20",
        "-fkiln-pch-snapshot=check",
        "-include-pch",
        "a.pch",
        "-S",
        "-emit-llvm",
        "a.cpp",
        "-o",
        "-",
    ];
    assert_eq!(ok(&d, &check).stderr, "");
}

/// WP-1369 found (pre-existing, 1.1.0) and fixed: a using-directive in
/// the body of a function template (libc++'s `<atomic>` and `<cmath>`
/// reach one once their functions declared through a typedef are
/// carried) has no `DEPENDENT_CTX` in sema, which the reader required of
/// every local of a template's pattern, so the header was replayed ("an
/// internal image inconsistency").
#[test]
fn local_using_directive_in_a_template_is_carried() {
    let header = "\
namespace N { inline int g(int x) { return x; } }
template <class T> int f(T t) { using namespace N; return g(t); }
template <class T> struct S { int h(T t) { using namespace N; return g(t) + 1; } };
inline int u() { return f(1) + S<int>{}.h(2); }
";
    let tu = "int main() { return u() - 4; }\n";
    snapshot_agrees("wp1369u", header, tu);
}
