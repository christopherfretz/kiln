// [dcl.pre]/12, [temp.res]: a failed requirement prints a qualified name's qualifier with its template arguments resolved and canonical, and explicit template arguments canonical, as clang does (WP-303)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:15 error "static assertion failed due to requirement 'n::tr<int, int>::value'"
//@ diag: 32:15 error "static assertion failed due to requirement 'tr<n::A, int>::value'"
//@ diag: 33:15 error "static assertion failed due to requirement 'tr<A, int>::In::w'"
//@ diag: 34:15 error "static assertion failed due to requirement 'n::vt<n::tr<n::A, int>>'"
//@ diag: 35:15 error "static assertion failed due to requirement 'n::V<3>::value'"
//@ diag: 36:15 error "static assertion failed due to requirement 'n::tr<int, long>::value'"
//@ diag: 37:15 error "static assertion failed due to requirement 'ok<int>(0)'"
//@ diag: 38:15 error "static assertion failed due to requirement 'tr<n::A, int>::ok(0)'"
//@ diag: 39:15 error "static assertion failed due to requirement 'tr<n::A, int>::ok(0)'"
//@ diag: 41:17 error "static assertion failed due to requirement 'n::tr<n::A, int>::value || n::vt<n::A *>': f"
//@ diag: 44:15 note "in instantiation of function template specialization 'f<n::A>' requested here"
//@ diag: 42:17 error "static assertion failed due to requirement 'ok<n::A>(0)': g"
//@ requires: clang<22
// clang 19 spelling; static-assert-substituted-names-clang22.cpp has clang 22's (WP-377).
namespace n {
struct A {};
template <class T, class U = int> struct tr {
  static constexpr bool value = false;
  struct In { static constexpr bool w = false; };
  template <class V> static constexpr bool ok(int) { return false; }
};
template <class T> constexpr bool vt = false;
template <int N> struct V { static constexpr bool value = false; };
}
using X = int;
using Y = n::tr<n::A>;
template <class T> constexpr bool ok(int) { return false; }
static_assert(n::tr<X>::value);
static_assert(Y::value);
static_assert(Y::In::w);
static_assert(n::vt<Y>);
static_assert(n::V<1 + 2>::value);
static_assert(n::tr<decltype(1 + 1), long>::value && true);
static_assert(ok<X>(0));
static_assert(Y::ok<X>(0));
static_assert(Y::template ok<X>(0));
template <class T> void f(T) {
  static_assert(n::tr<T>::value || n::vt<T *>, "f");
  static_assert(ok<T>(0) && sizeof(T) == 1, "g");
}
template void f<n::A>(n::A);
