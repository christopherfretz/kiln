// [expr.prim.req.general]/5: substitution into the requirements proceeds
// in lexical order and stops at the first that decides the result. In a
// generic lambda of a function template the enclosing template's
// arguments are known first: a requirement that names the lambda's own
// parameter stays dependent, and the requirements after it are not
// substituted until it is known. `typename U::type` failing for
// `U = int` makes the expression false before `S<T>::type` is formed, so
// the hard error in `S<int>` is never reached. GCC 16.1 agrees; clang 19
// and 22 substitute `S<T>::type` while instantiating `f<int>` and reject
// (the text decides). Found running GCC's C++ testsuite.
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19/22 substitute the later requirement of a generic lambda's requires-expression early
//@ strict-diags
template <typename T> struct S { using type = T::type; };
struct HasType { using type = int; };
struct HasX { int x; };
template <class T> concept Small = sizeof(T) <= 4;

// The lambda's template parameter first, the enclosing one after it.
template <typename T> constexpr auto f() {
  return []<typename U>(U) { return requires { typename U::type; typename S<T>::type; }; };
}
static_assert(!f<int>()(0));
static_assert(f<HasType>()(HasType{}));

// The lambda's parameter, a capture and locals of both functions.
template <typename T> constexpr auto g(T t) {
  int n = 0;
  return [t, n](auto u) {
    long m = 0;
    return requires { u.x; t + n + m + u.x; { u.x } -> Small; typename S<T>::type; };
  };
}
static_assert(!g(0)(0));
static_assert(!g(HasType{})(HasX{}));  // `HasType + int` is invalid
static_assert(g(HasType{}).operator()(HasX{}) == false);

// Nested generic lambdas: two partial substitutions.
template <typename T> constexpr auto h() {
  return [](auto a) { return [](auto b) { return requires { a.x; b.x; typename S<T>::type; }; }; };
}
static_assert(!h<int>()(0)(HasX{}));
static_assert(!h<int>()(HasX{})(0));

// A constexpr-if condition and the lambda's requires-clause.
template <typename T> constexpr int k() {
  auto l = [](auto u) {
    if constexpr (requires { u.x; typename S<T>::type; })
      return 1;
    else
      return 0;
  };
  return l(0);
}
static_assert(k<int>() == 0);
template <typename T> constexpr auto r() {
  return []<typename U>(U) requires(!requires { typename U::type; typename S<T>::type; }) { return 0; };
}
static_assert(r<int>()(0) == 0);

// A compound requirement's type-constraint is checked only once its
// expression is valid ([expr.prim.req.compound]/1).
template <typename T> constexpr auto c() {
  return [](auto u) { return requires { { u.x } -> Small; typename S<T>::type; }; };
}
static_assert(!c<int>()(0));

// Default captures, an enclosing pack expanded in a requirement, and
// the requires-expression's own parameters.
template <typename T> constexpr auto g1(T t) {
  return [=](auto u) { return requires { u.x; t.y; typename S<T>::type; }; };
}
static_assert(!g1(0)(HasX{}) && !g1(0)(0));
template <typename T> constexpr auto g2(T t) {
  return [&](auto u) { return requires { u.x; t + 1; }; };
}
static_assert(g2(0)(HasX{}) && !g2(0)(0));
template <typename... Ts> constexpr auto p() {
  return [](auto u) { return requires { u.x; (Ts::x, ...); }; };
}
static_assert(!p<int>()(HasX{}) && p<HasX>()(HasX{}));
template <typename T> constexpr auto q() {
  return [](auto u) { return requires(decltype(u) v, T w) { v.x; w + 1; }; };
}
static_assert(q<int>()(HasX{}) && !q<HasX>()(HasX{}));

// A default member initializer of a class template: no partial
// substitution, the same order.
template <typename T> struct D { bool b = requires { typename T::type; typename S<T>::type; }; };
static_assert(!D<int>{}.b);
