// [expr.prim.req.general]/5, [temp.variadic]/7: substitution into the
// requirements stops at the first that decides the result, also when the
// generic lambda holding the requires-expression is in a pack expansion
// (a fold, a pack-expanded base initializer, an initializer list): each
// element of the expansion leaves its requirements after the one that
// names the lambda's parameter unsubstituted until the lambda is called,
// and substitutes them for its own element of the pack then. The hard
// error in `S<int>` is never reached when `typename U::type` fails; when
// it holds, `Ts` is the element's (`T1` and `T2` differ). The enclosing
// pack named through `sizeof...` and a pack index `Ts...[0]` is no
// different. The text decides: clang 19 and 22 substitute the later
// requirements early (see p5-generic-lambda-requirement-order.cpp), and
// GCC 16.1, which orders them as kiln does outside an expansion, rejects
// these lambdas in a pack expansion ("parameter packs not expanded").
// Found running GCC's C++ testsuite (WP-1599).
//@ std: c++26
//@ skip(clang): clang 19/22 substitute the later requirement of a generic lambda's requires-expression early
//@ strict-diags
template <typename T> struct S { using type = T::type; };
struct HasType { using type = int; };
struct T1 { using type = int; };
struct T2 {};

// In a fold.
template <typename... Ts> constexpr int fold(auto u) {
  return (0 + ... + ([]<typename U>(U) { return requires { typename U::type; typename S<Ts>::type; }; }(u) ? 1 : 0));
}
static_assert(fold<int, long>(0) == 0);
template <typename... Ts> constexpr int which(auto u) {
  return (0 + ... + ([]<typename U>(U) { return requires { typename U::type; typename Ts::type; }; }(u) ? 1 : 0));
}
static_assert(which<T1, T2, T1>(HasType{}) == 2);
static_assert(which<T1, T2, T1>(0) == 0);

// In a pack-expanded base initializer.
template <class T> struct B {
  bool v;
  template <class L> constexpr B(L l, auto u) : v(l(u)) {}
};
template <typename... Ts> struct D : B<Ts>... {
  constexpr D(auto u)
      : B<Ts>([]<typename U>(U) { return requires { typename U::type; typename S<Ts>::type; }; }, u)... {}
  constexpr int count() const { return (0 + ... + (B<Ts>::v ? 1 : 0)); }
};
static_assert(D<int, long>(0).count() == 0);
static_assert(D<T1, HasType>(HasType{}).count() == 2);

// In an initializer list.
template <typename... Ts> struct E {
  bool b[sizeof...(Ts)] = {[]<typename U>(U) { return requires { typename U::type; typename S<Ts>::type; }; }(0)...};
};
static_assert(!E<int, long>{}.b[0] && !E<int, long>{}.b[1]);

// With `sizeof...` and a pack index of the enclosing pack.
template <typename... Ts> constexpr int size(auto u) {
  return (0 + ... + ([]<typename U>(U) { return requires { typename U::type; typename S<int[sizeof...(Ts)]>::type; }; }(u) ? sizeof(Ts) : 0));
}
static_assert(size<int, long>(0) == 0);
template <typename... Ts> constexpr int index(auto u) {
  return (0 + ... + ([]<typename U>(U) { return requires { typename U::type; typename S<Ts...[0]>::type; }; }(u) ? sizeof(Ts) : 0));
}
static_assert(index<int, long>(0) == 0);
template <typename... Ts> constexpr int index1(auto u) {
  return (0 + ... + ([]<typename U>(U) { return requires { typename U::type; typename Ts...[1]::type; }; }(u) ? sizeof(Ts) : 0));
}
static_assert(index1<T2, T1>(HasType{}) == 2 && index1<T1, T2>(HasType{}) == 0);
