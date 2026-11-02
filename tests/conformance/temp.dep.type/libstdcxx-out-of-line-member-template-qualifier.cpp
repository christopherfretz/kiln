// [temp.dep.type]/6, [temp.over.link]/5: in the return type written before
// the declarator-id of an out-of-line member of class template V,
// `typename V<T>::template U<A>::R` names the member template U of the
// current instantiation, so it matches the in-class `typename U<A>::R`.
// libstdc++ valarray:1100 (`typename valarray<_Tp>::template
// _UnaryOp<_Name>::_Rt valarray<_Tp>::operator+() const`); <valarray>
// (docs/status/libstdcxx.md, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 3 4 5
#include <stdio.h>
struct P {};
template<class T> class V {
  template<class O> struct U { typedef T R; };
  template<class O> using A = U<O>;
public:
  typename U<P>::R f() const;
  typename V<T>::template U<P>::R g() const;
  typename A<P>::R h() const;
};
template<class T>
typename V<T>::template U<P>::R V<T>::f() const { return 3; }
template<class T>
typename V<T>::template U<P>::R V<T>::g() const { return 4; }
template<class T>
typename V<T>::template A<P>::R V<T>::h() const { return 5; }

int main() {
  V<int> v;
  printf("%d %d %d\n", v.f(), v.g(), v.h());
}
