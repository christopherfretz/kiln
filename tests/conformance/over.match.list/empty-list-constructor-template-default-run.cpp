// [dcl.init.list]/3.5, [over.match.list]/1: an empty braced-init-list
// value-initializes a class with a default constructor, which a
// constructor template callable with no argument is
// ([class.default.ctor]/1, clang `LookupDefaultConstructor`): `A a{}`
// calls `A<>()`, as in an instantiated template.
//@ expect: run
//@ stdout: 0 0 2 0
#include <cstdio>
struct A {
  int n;
  template<class... T> A(T... t) : n(sizeof...(T)) {}
};
struct V {
  int n = 9;
  template<class... T> V(T...) = delete;
  V(int i) : n(i) {}
};
template<class T> int f() { T t{}; return t.n; }
int main() {
  A a{};
  A b = {};
  A c{1, 2};
  std::printf("%d %d %d %d\n", a.n, b.n, c.n, f<A>());
}
