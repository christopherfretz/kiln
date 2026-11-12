export module imgltd;
export namespace ltd {
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
