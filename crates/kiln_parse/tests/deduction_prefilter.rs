//! WP-657: the call-candidate deduction pre-filters (spec/sema-templates.md §6.1)
//! change no result. Each program is analysed twice, with the pre-filters
//! and with every deduction run in full (`ParseOptions::full_deduction`,
//! the test-only entry), and the two must agree on every diagnostic and
//! note (the candidate notes name each candidate's deduction failure and
//! keep their order) and on the AST dump (the chosen candidates and every
//! specialization the resolutions formed). `kiln_sema` has no parser, so the
//! comparison lives with the parser's sema tests.

mod common;

use std::fmt::Write as _;

use std::sync::Arc;

use common::{Parsed, opts19, parse_with};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::ParseOptions;
use kiln_sema::target::{Arch, Target};
use kiln_sema::templ::report::ReportFormat;
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Analyses `src` both ways and asserts they agree; returns the
/// pre-filtered result.
#[track_caller]
fn same_both_ways(src: &str) -> Parsed {
    let fast = parse_with(src, &opts19());
    let full = parse_with(src, &ParseOptions { full_deduction: true, ..opts19() });
    assert_eq!(fast.diags, full.diags, "diagnostics differ with the pre-filters");
    assert_eq!(fast.dump, full.dump, "AST dump differs with the pre-filters");
    fast
}

/// Too few arguments for templates with packs, defaults and explicit
/// arguments, beside viable ones: the arity failures are the full path's
/// and the right candidate wins.
#[test]
fn arity_failures_match_the_full_path() {
    let p = same_both_ways(
        r"
template <class T> int f(T, T);
template <class T, class U = int> int f(T, U, U = U());
template <class... Ts> int f(int, int, int, Ts...);
template <class T, int N> int f(T (&)[N], int, int, int);
int f(long);
int a[3];
int x1 = f(1);
int x2 = f(1, 2);
int x3 = f(1, 2, 3);
int x4 = f(a, 1, 2, 3);
int x5 = f<int>(1);
int x6 = f<int, long>(1, 2);
void g() { f(); }
",
    );
    p.assert_diag("error: no matching function for call to 'f'");
    p.assert_diag("requires at least 2 arguments, but 0 were provided");
}

/// Explicit arguments of the wrong kind or value, non-dependent
/// parameters that do not convert, constraints, SFINAE'd and deleted
/// candidates, and an ambiguity, each noted.
#[test]
fn deduction_failures_match_the_full_path() {
    let p = same_both_ways(
        r"
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
struct S {};
template <class T> concept Small = sizeof(T) <= 4;
template <class T> typename enable_if<sizeof(T) == 1, int>::type h(T);
template <class T> int h(T, S);
template <Small T> int h(T, int);
template <int N> int h(int, int, int);
template <class T> int h(T, T, T) = delete;
template <class T> int k(T, long);
template <class T> int k(long, T);
void use() {
  h(1, S());
  h(1, 2);
  h(1.0, 2);
  h<3>(1, 2, 3);
  h<int>(1, 2, 3);
  h('c');
  h(S(), 1);
  k(1, 2);
}
",
    );
    p.assert_diag("call to deleted function 'h'");
    p.assert_diag("call to 'k' is ambiguous");
    p.assert_diag("candidate template ignored");
}

/// Member templates, an explicit object parameter, constructors and
/// operator templates: the leading object argument counts toward the
/// arity as in the full path.
#[test]
fn object_and_operator_candidates_match_the_full_path() {
    let p = same_both_ways(
        r"
struct A {
  template <class T> A(T, T);
  template <class T> A(T);
  template <class T, class U> int m(T, U);
  template <class T> int m(T);
  template <class Self> int e(this Self&&, int);
  template <class Self, class T> int e(this Self&&, T, T);
};
template <class T> bool operator==(const A&, T);
template <class T, class U> bool operator==(T, U, int);
void use(A a) {
  A b(1, 2);
  A c(1);
  A d(1, 2, 3);
  a.m(1);
  a.m(1, 2);
  a.m(1, 2, 3);
  a.e(1);
  a.e(1, 2);
  a.e();
  bool q = a == 1;
}
",
    );
    p.assert_diag("no matching constructor for initialization of 'A'");
    p.assert_diag("no matching member function for call to 'm'");
}

/// A generated set: templates of 0–3 parameters with and without packs
/// and defaults, called with 0–4 arguments, with and without explicit
/// arguments.
#[test]
fn generated_calls_match_the_full_path() {
    let mut src = String::from("struct Z {};\n");
    let decls = [
        "template <class T> int g(T);",
        "template <class T> int g(T, T);",
        "template <class T, class U> int g(T, U, int = 0);",
        "template <class... Ts> int g(Ts...);",
        "template <class T, class... Ts> int g(T, T, Ts...);",
        "template <int N, class T> int g(T, Z);",
        "template <class T> int g(Z, T, T, T);",
    ];
    for d in decls {
        src.push_str(d);
        src.push('\n');
    }
    let args = ["", "1", "1, 2", "1, Z()", "Z(), 1, 2, 3", "1, 2, 3, 4"];
    let explicit = ["", "<int>", "<1>", "<int, long>"];
    for (i, a) in args.iter().enumerate() {
        for (j, e) in explicit.iter().enumerate() {
            writeln!(src, "auto v{i}_{j} = g{e}({a});").unwrap();
        }
    }
    let p = same_both_ways(&src);
    p.assert_diag("candidate template ignored");
}

/// Explicit non-type arguments that are parameters of an enclosing
/// specialization (`get<I>` inside `tuple<...>`): integral, signed,
/// negative, wide, `bool`, `char` and enumeration values, through
/// typedefs and `const`, converted to the parameter's type or narrowed,
/// form the same specializations and the same failures as the full
/// conversion.
#[test]
fn substituted_non_type_arguments_match_the_full_path() {
    let p = same_both_ways(
        r"
typedef unsigned long size_t;
enum E { e0, e1 };
enum class F : short { f0 = -3 };
template <size_t I> int get(int (&)[I + 1]) { return I; }
template <size_t I> int get(long) { return int(I); }
template <int I> int sget() { return I; }
template <long long I> int wget() { return int(I >> 40); }
template <unsigned char I> int nget() { return I; }
template <bool B> int bget() { return B; }
template <char C> int cget() { return C; }
template <E V> int eget() { return V; }
template <F V> int fget() { return int(V); }
template <size_t I, int J, long long K, bool B, char C, E V, F W> struct A {
  static int run() {
    int a[I + 1] = {};
    return get<I>(a) + get<I>(0L) + sget<J>() + sget<I>() + wget<K>()
         + bget<B>() + cget<C>() + eget<V>() + fget<W>() + sget<V>();
  }
  static int narrow() { return nget<J>(); }
};
template <const size_t I> struct C { static int run() { return get<I>(0L) + sget<int(I)>(); } };
int r1 = A<3, -7, (1LL << 50), true, 'x', e1, F::f0>::run();
int r2 = A<0, 2, -1, false, '\0', e0, F::f0>::run();
int r3 = C<5>::run();
int r4 = A<1, 300, 0, true, 'a', e0, F::f0>::narrow();
",
    );
    // The narrowed `300` is a deduction failure, the only error.
    p.assert_diag("error: no matching function for call to 'nget'");
    assert_eq!(p.diags.iter().filter(|d| d.contains("error:")).count(), 1, "{:#?}", p.diags);
}

/// The NDJSON instantiation report of `src` (WP-661), without its timings,
/// one line per row, sorted.
fn report(src: &str, full_deduction: bool) -> Vec<String> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { full_deduction, inst_report: ReportFormat::Json, ..opts19() };
    let tu = kiln_parse::parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        Target::for_arch(Arch::X86_64),
        &opts,
        kiln_parse::ConstexprLimits::default(),
    );
    let mut rows: Vec<String> = tu
        .inst_report
        .lines()
        .map(|l| {
            let mut out = String::new();
            for f in l.trim_matches(|c| c == '{' || c == '}').split(',') {
                if !f.starts_with("\"self_us\"") && !f.starts_with("\"incl_us\"") {
                    let _ = write!(out, "{f},");
                }
            }
            out
        })
        .collect();
    rows.sort();
    rows
}

/// WP-661: the explicit-argument substitution memo changes nothing. The
/// same `(template, explicit arguments)` pairs are deduced many times:
/// successes (answered from the memo), SFINAE failures (never kept: each
/// is noted again, at the same call), a failure that later succeeds once
/// the class it looks into is complete, a substitution that instantiates
/// a class (not kept: it created nodes), packs, the address of a
/// specialization, and a generic lambda (a local-scope template, never
/// kept). Diagnostics, the AST and every instantiation-report row
/// (count, nodes, bytes) agree with the memo off.
#[test]
fn explicit_argument_memo_matches_the_full_path() {
    let src = r"
template <class T, class U> T conv(U u) { return T(u); }
template <class T> struct box { using type = T; T v; };
template <class T> int unbox(typename box<T>::type t, T) { return int(t); }
template <class T> typename T::type pick(T) { return 0; }
template <class T> int pick(...) { return 1; }
template <class... Ts> int count(int, Ts...) { return sizeof...(Ts); }
template <class T, class... Ts> int first(T t, Ts...) { return int(t); }
struct S;
template <class T> int deref(typename T::type*) { return 2; }
int c1 = conv<int>(1L) + conv<int>(2L) + conv<long>(3) + conv<int>(4L);
int c2 = unbox<int>(1, 2) + unbox<int>(3, 4) + unbox<long>(5, 6L);
int c3 = pick<int>(0) + pick<int>(1) + pick<box<int>>(box<int>{});
int c4 = count<long, char>(0, 1, 'a') + count<long, char>(1, 2, 'b') + count<long>(2, 3);
int c5 = first<int, long>(1, 2L) + first<int, long>(3, 4L) + first<int>(5);
int (*p1)(long) = &conv<int, long>;
int (*p2)(long) = &conv<int, long>;
int e1 = deref<S>(0);
struct S { using type = int; };
int e2 = deref<S>(0) + deref<S>(0);
int c6 = pick<char>(0) + pick<char>(0);
void g() {
  auto l = []<class T>(T t) { return int(t); };
  int a = l.operator()<long>(1) + l.operator()<long>(2);
  (void)a;
}
";
    let p = same_both_ways(src);
    p.assert_diag("error: no matching function for call to 'deref'");
    assert_eq!(p.diags.iter().filter(|d| d.contains("error:")).count(), 1, "{:#?}", p.diags);
    let (fast, full) = (report(src, false), report(src, true));
    assert!(fast.iter().any(|r| r.contains("\"kind\":\"default\"")), "{fast:#?}");
    assert_eq!(fast, full, "instantiation report differs with the memo");
}

/// WP-661: the minimum argument count follows default arguments that later
/// redeclarations add ([dcl.fct.default]/4), out-of-class member
/// definitions add (/6) and class-scope late parsing supplies: each call
/// is checked against the defaults visible at it, after the same
/// functions were already asked for their minimum (a function template
/// cannot gain defaults, /4, so it is only asked repeatedly). The arity
/// failures keep their notes.
#[test]
fn min_args_follow_added_default_arguments() {
    let p = same_both_ways(
        r"
void f(int, int, int);
int a1 = (f(1, 2, 3), 0);
void f(int, int, int = 3);
int a2 = (f(1, 2), 0);
void f(int, int = 2, int);
int a3 = (f(1), 0);
template <class T> int t(T, int, int = 3);
int b1 = t(1, 2, 3) + t(1, 2) + t<long>(1, 2) + t<long>(1, 2, 3);
struct S {
  int m(int, int);
  int n(int = 1) { return m(1, 2) + k(); }
  int k(int = 0) { return 0; }
  int u(int, int) { return 0; }
};
int c1 = S().m(1, 2) + S().n() + S().k();
int S::m(int, int = 2) { return 0; }
int c2 = S().m(1);
void g(int, int);
void use() { g(1); }
void g(int, int = 0);
void use2() { g(1); g(); }
",
    );
    p.assert_diag("error: no matching function for call to 'g'");
    assert_eq!(p.diags.iter().filter(|d| d.contains("error:")).count(), 2, "{:#?}", p.diags);
}
