// WP-1369: the forms of spec/modules.md §11.11 "Forms carried by
// WP-1369", exported for image-typedef-fns-run.cpp and
// image-typedef-fns-errors.cpp.
export module imgtf;
export namespace tf {
typedef int F(int);
typedef int N(int) noexcept;
template <class T> using Un = T(T);
F twice;
int twice(int v = 4) { return v * 2; }
N thrice;
int thrice(int v) noexcept { return v * 3; }
struct Q {
  F inc;
  F dec;
};
int Q::inc(int a) { return a + 1; }
int Q::dec(int a) { return a - 1; }
template <class T> struct Box {
  Un<T> half;
  static N quad;
};
template <class T> T Box<T>::half(T t) { return t / 2; }
template <class T> int Box<T>::quad(int x) noexcept { return x * 4; }
int base_fn(long v) { return int(v); }
decltype(base_fn) dt_fn;
int dt_fn(long v) { return int(v) + 1; }
__typeof(base_fn) to_fn;
int to_fn(long v) { return int(v) + 2; }

// A member of a base of an implicit instantiation, and its member class.
template <class T> struct counter {
  T n = 0;
  T bump() { return ++n; }
};
template <int N> struct storage {
  struct type {
    alignas(N) unsigned char b[N];
  };
};
template <class T> struct holder : counter<T> {
  typename storage<sizeof(T)>::type s;
};
int use_holder() {
  holder<long> h;
  h.bump();
  return int(h.bump()) + int(sizeof(h.s));
}
} // namespace tf
