export module imgpi;
export namespace pi {
template <class T> struct Base {
  T b;
  Base(T x) : b(x) {}
};
template <class T> struct Iter {
  T *p;
  Iter() : p(nullptr) {}
  explicit Iter(T *q) : p(q) {}
  template <class U> Iter(const Iter<U> &o) : p(o.p) {}
};
template <class T> struct Derived : Base<T> {
  typedef Base<T> base_type;
  int k;
  int arr[2];
  Derived(T x) : base_type(x), k{3}, arr{1, 2} {}
  Derived() : Derived(T(7)) {}
};
template <class... Bs> struct Multi : Bs... {
  Multi(int v) : Bs(v)... {}
};
struct B1 { int a; B1(int x) : a(x) {} };
struct B2 { int c; B2(int x) : c(x + 1) {} };
template <class T> struct Out {
  T v;
  Out(T);
};
template <class T> Out<T>::Out(T x) : v(x * 2) {}
inline int local_attrs() {
  [[maybe_unused]] int a = 3;
  __attribute__((unused)) int b = 4;
  return a + b;
}
}
