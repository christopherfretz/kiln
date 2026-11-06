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

use common::{Parsed, opts19, parse_with};
use kiln_parse::ParseOptions;

/// Analyses `src` both ways and asserts they agree; returns the
/// pre-filtered result.
#[track_caller]
fn same_both_ways(src: &str) -> Parsed {
    let fast = parse_with(src, opts19());
    let full = parse_with(src, ParseOptions { full_deduction: true, ..opts19() });
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
