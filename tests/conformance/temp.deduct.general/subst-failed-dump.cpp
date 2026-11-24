//@ expect: ast
//@ std: c++26
//@ args: -fkiln-verify-subst-failed
//@ ast-filter: sf_
//@ skip(clang): the subst-failed token and the specializations that carry it are kiln's own (clang forms none of them)
// [temp.deduct.general]/7, /8, [temp.spec.partial.match]/1: a
// specialization kiln forms and makes invalid while the deduction traps
// every error carries `subst-failed=<kind>` after `invalid`
// (spec/ast-dump.md §3, spec/ast.md §4.1 "Silent failures"); the
// translation unit has no error, and the checker finds every invalid
// declaration marked. A statement-expression's local in a failed
// signature (kind 5) is a member of a closure, which the dump does not
// print: crates/kiln_parse/tests/subst_failed.rs covers it.

// Kind 1: the trailing return type fails to substitute; the
// specialization has the error type and no parameters.
template <class T> auto sf_g(T t) -> decltype(t.size());
int sf_g(...);

// Kind 1: the signature substitutes, but the frame trapped a narrowing.
template <class T> auto sf_h(T t) -> decltype(int{t});
long sf_h(...);

// Kind 2: only the explicit-specifier fails.
struct sf_S {
  template <class T> explicit(T::value) sf_S(T) {}
  sf_S(...) {}
};

// Kind 3: the variable partial-specialization choice is ambiguous.
template <class T, class U> constexpr int sf_v = 0;
template <class T> constexpr int sf_v<T, int> = 1;
template <class U> constexpr int sf_v<int, U> = 2;
template <class T> auto sf_f(T) -> decltype(sf_v<T, int>);
long sf_f(...);

// Kind 4: the class partial-specialization choice is ambiguous.
template <class T, class U> struct sf_X {};
template <class T> struct sf_X<T, int> { using t = int; };
template <class U> struct sf_X<int, U> { using t = int; };
template <class T> auto sf_k(T) -> typename sf_X<T, int>::t;
long sf_k(...);

int a = sf_g(1);
long b = sf_h(1.5);
sf_S s = 1;
long c = sf_f(1);
long d = sf_k(1);
