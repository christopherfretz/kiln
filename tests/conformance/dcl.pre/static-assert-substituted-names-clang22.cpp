// [dcl.pre]/12, [temp.res]: a failed requirement prints a qualified name's qualifier with its template arguments resolved and canonical, and explicit template arguments canonical, as clang does (WP-303)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:15 error "static assertion failed due to requirement 'n::tr<int, int>::value'"
//@ diag: 34:15 error "static assertion failed due to requirement 'n::tr<n::A, int>::value'"
//@ diag: 35:15 error "static assertion failed due to requirement 'n::tr<n::A, int>::In::w'"
//@ diag: 36:15 error "static assertion failed due to requirement 'n::vt<n::tr<n::A, int>>'"
//@ diag: 37:15 error "static assertion failed due to requirement 'n::V<3>::value'"
//@ diag: 38:15 error "static assertion failed due to requirement 'n::tr<int, long>::value'"
//@ diag: 39:15 error "static assertion failed due to requirement 'ok<int>(0)'"
//@ diag: 40:15 error "static assertion failed due to requirement 'n::tr<n::A, int>::ok(0)'"
//@ diag: 41:15 error "static assertion failed due to requirement 'n::tr<n::A, int>::ok(0)'"
//@ diag: 43:17 error "static assertion failed due to requirement 'n::tr<n::A, int>::value || n::vt<n::A *>': f"
//@ diag: 46:15 note "in instantiation of function template specialization 'f<n::A>' requested here"
//@ diag: 44:17 error "static assertion failed due to requirement 'ok<n::A>(0)': g"
//@ diag: 50:15 error "static assertion failed due to requirement '(anonymous namespace)::q<int>::v'"
//@ diag: 51:15 error "static assertion failed due to requirement 'r<int>::v'"
//@ requires: clang>=22
// clang 22 spelling of static-assert-substituted-names.cpp (WP-377).
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

namespace { template <class T> struct q { static constexpr bool v = false; }; }
inline namespace i { template <class T> struct r { static constexpr bool v = false; }; }
static_assert(q<int>::v);
static_assert(r<int>::v);
