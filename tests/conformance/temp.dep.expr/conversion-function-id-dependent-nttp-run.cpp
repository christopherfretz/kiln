// [temp.dep.expr]/3 with a conversion to a type parameterised by a
// non-type template argument (libstdc++ <experimental/simd>'s
// `operator __vector_type_t<value_type, _Np * 4>()`, simd.h:2271): a
// conversion function template deduced from `Arr<T, N>` ([temp.deduct.conv])
// named with the member template's own `N`, and a non-template conversion
// to `V_t<T, 2> *` whose canonical name keeps the literal `2` as an
// expression to substitute (WP-1126).
//@ expect: run
//@ std: c++20
//@ stdout: 3 2 1
#include <cstdio>
template <class T, int N> struct V { using type = T[N]; };
template <class T, int N> using V_t = typename V<T, N>::type;
template <class T, int N> struct Arr { T a[N]; };
template <class T> struct Proxy {
  Arr<T, 4> store{{1, 2, 3, 4}};
  template <int N> operator Arr<T, N>() const {
    Arr<T, N> r{};
    for (int i = 0; i < N; ++i) r.a[i] = store.a[i];
    return r;
  }
  operator V_t<T, 2> *() const { return nullptr; }
  template <int N> int take() const { return operator Arr<T, N>().a[N - 1]; }
  template <int N> int take2() const { return this->operator Arr<T, N>().a[N - 1]; }
  bool z() const { return operator V_t<T, 2> *() == nullptr; }
};
int main() {
  Proxy<int> p;
  std::printf("%d %d %d\n", p.take<3>(), p.take2<2>(), p.z() ? 1 : 0);
}
