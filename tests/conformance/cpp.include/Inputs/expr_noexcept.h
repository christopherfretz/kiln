#ifndef EXPR_NOEXCEPT_H
#define EXPR_NOEXCEPT_H
inline int nx_sized() noexcept(sizeof(int) > 2) { return 1; }
template <class T> int nx_make() noexcept(noexcept(T())) { return (void)T(), 2; }
template <class T> struct nx_pair {
  T a;
  void swap(nx_pair &o) noexcept(noexcept(a = o.a)) {
    T t = a;
    a = o.a;
    o.a = t;
  }
};
template <class T> void swap(nx_pair<T> &x, nx_pair<T> &y) noexcept(noexcept(x.swap(y))) {
  x.swap(y);
}
inline int nx_add(int &x) noexcept(noexcept(x + 1)) { return x + 1; }
#endif
