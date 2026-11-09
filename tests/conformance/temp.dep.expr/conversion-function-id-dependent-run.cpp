// [temp.dep.expr]/3: an id-expression that is a conversion-function-id
// naming a dependent type is type-dependent, so `operator V_t<T, N> *()`
// in a class template's member is looked up (as `this->operator ...()`)
// at instantiation, not at definition ([temp.res.general]/9). libstdc++
// 14's <experimental/simd> (`_ZeroExtendProxy`, simd.h:2271) has this
// shape in a member template it never instantiates; kiln rejects it on
// both arches (found by the WP-1002 x86_64 libstdc++ sweep). The canonical
// `V<T, 2>` keeps `2` as an expression profile, which substitution now
// turns back into its expression (WP-1126).
//@ expect: run
//@ std: c++20
//@ stdout: 1 1
#include <cstdio>
template <class T, int N> struct V { using type = T[N]; };
template <class T, int N> using V_t = typename V<T, N>::type;
template <class T> struct Proxy {
  T x;
  operator V_t<T, 2> *() const { return nullptr; }
  bool f() const { return operator V_t<T, 2> *() == nullptr; }
  template <int N> bool g() const { return operator V_t<T, N> *() == nullptr; }
};
int main() {
  Proxy<int> p{3};
  std::printf("%d %d\n", p.f() ? 1 : 0, p.g<2>() ? 1 : 0);
}
