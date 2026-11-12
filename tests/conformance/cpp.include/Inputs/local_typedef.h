// Block-scope typedef and alias declarations in function templates,
// member functions of class templates and inline functions (the shape of
// libc++'s <__algorithm/ranges_iterator_concept.h>; WP-1314).
#ifndef LOCAL_TYPEDEF_H
#define LOCAL_TYPEDEF_H
namespace ltd {
template <class T> struct box { T v; };
template <class T> long f(T t) {
  typedef box<T> B;
  using P = const T *;
  B b{t};
  P p = &b.v;
  return long(*p) + long(sizeof(B));
}
inline int g(int x) {
  typedef int I;
  using L = long;
  I i = x;
  return int(L(i) * 2);
}
template <class T> struct S {
  static int h() {
    using U = typename T::type;
    U u{};
    return int(sizeof(u));
  }
};
} // namespace ltd
#endif
