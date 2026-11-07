// [temp.dep.expr]/3: `auto(x)`, `auto{x}` and a CTAD `W(x)` are
// type-dependent when an argument is, since the deduced type comes from
// it; a non-placeholder `T(x)` with non-dependent `T` is only
// value-dependent (functional-cast-not-type-dependent-run.cpp). So
// `sizeof(auto(t))` and `auto x = auto{t};` wait for instantiation.
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++23
//@ stdout: 4 4 1 8 7
#include <cstdio>
template <class T> struct W {
  T v;
  W(T x) : v(x) {}
};
struct S {
  int a;
};
template <class T> unsigned long f(T t) { return sizeof(auto(t)); }
template <class T> unsigned long g(T t) { return sizeof(W(t)); }
template <class T> int h(T t) {
  auto x = auto{t};
  return x.a;
}
int main() {
  std::printf("%lu %lu %lu %lu %d\n", f(1), f(S{2}), g('c'), g(2.0), h(S{7}));
}
