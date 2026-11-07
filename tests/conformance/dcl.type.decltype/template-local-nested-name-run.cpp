// [temp.dep.type]/7, [temp.dep.expr]/3: in a template, `decltype(V)` of
// a local variable or parameter whose type is not dependent is not a
// dependent type, so `decltype(V)::iterator` names a member of a known
// class and is a type where a template argument is (no `typename`;
// clang `computeDependence(DeclRefExpr *)`; LLVM's MappedIteratorTest.cpp
// `mapped_iterator<decltype(V)::iterator, FuncT>`). A dependent local
// (`T t`) still is, and its use waits for instantiation.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 7 7 3 9 1 2
#include <cstdio>
#include <type_traits>
#include <vector>
template <class I, class F> struct M { I it; F f; int get() const { return f(*it); } };
template <class T> int with_local(T add) {
  std::vector<int> V({4, 5});
  using IterT = M<decltype(V)::iterator, T>;
  IterT m{V.begin(), add};
  return m.get();
}
template <class T> struct S {
  int g(std::vector<int> P) {
    using I2 = M<decltype(P)::const_iterator, T>;
    static_assert(std::is_same<decltype((P)), std::vector<int> &>::value, "");
    I2 m{P.cbegin(), T{}};
    return m.get();
  }
};
struct Inc { int operator()(int x) const { return x + 1; } };
template <class T> int dependent_local() {
  T t{};
  return static_cast<int>(sizeof(typename decltype(t)::value_type));
}
template <class T> int via_generic(T x) {
  std::vector<int> V(9);
  auto l = [&](auto u) {
    std::vector<decltype(V)::size_type> sizes{V.size()};
    return static_cast<int>(sizes[0]) + u;
  };
  return l(x);
}
int main() {
  auto add3 = [](int x) { return x + 3; };
  std::printf("%d %d %d %d %d %d\n", with_local(add3), with_local(add3), S<Inc>().g({2}),
              via_generic(0), dependent_local<std::vector<char>>(),
              dependent_local<std::vector<short>>());
}
