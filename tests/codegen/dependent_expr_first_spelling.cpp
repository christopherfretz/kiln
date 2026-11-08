// Itanium ABI §5.1.5: a value-dependent expression in a dependent type
// mangles as the declaration that first formed the canonical type spelled
// it (clang keeps that spelling's expressions in its canonical node):
// `R<T>::value` and `R<T, void>::value` are one expression, so `f`/`g`
// take `R<T>` (written first in `EI<..., T>`) while `g2`/`f2` take
// `R<T, void>` (written first in `EI<..., T *>`), and likewise per
// `decltype` and per dependent array bound. Both orders, one TU (WP-790).
template <class, class = void> struct R { static constexpr int value = 1; };
template <int, class T> struct EI { using type = T; };
template <int> struct A {};
template <class T> typename EI<R<T>::value, T>::type f(T) { return T(); }
template <class T> typename EI<R<T, void>::value, T>::type g(T) { return T(); }
template <class T> typename EI<R<T, void>::value, T *>::type g2(T) { return 0; }
template <class T> typename EI<R<T>::value, T *>::type f2(T) { return 0; }
template <class T> void m(A<R<T>::value + 1>) {}
template <class T> void n(A<R<T, void>::value + 1> *) {}
template <class T> void dg(decltype(R<T, void>::value + 1)) {}
template <class T> void dh(decltype(R<T>::value + 1)) {}
template <class T> void k(A<0>, int (*)[R<T, void>::value + 1]) {}
template <class T> void l(int (*)[R<T>::value + 1]) {}
int use() {
  m<int>({});
  n<int>(nullptr);
  dg<int>(0);
  dh<int>(0);
  k<int>({}, nullptr);
  l<int>(nullptr);
  return f(1) + g(2) + (f2(3) == g2(4));
}
