// The module of image-class-template-run.cpp (spec/modules.md §11.11
// stage S4, "Class template patterns"): class templates with in-class
// and out-of-line members, static data members, nested types, default
// template arguments and dependent bases, whose patterns the
// declaration image carries, and bodies naming specializations.
export module imgct;
export namespace ct {
constexpr long fib(long n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
template <class T> struct Box {
  T v;
  static constexpr int tag = 7;
  using value_type = T;
  constexpr T get() const { return v; }
  constexpr T twice() const { return v + get(); }
};
template <class T> struct Arr {
  T a;
  struct It {
    int i;
    T t;
    int get() const { return i; }
  };
  int size() const;
  static int count;
  Arr<T> *self() { return this; }
  It mk() const {
    It r;
    r.i = 3;
    r.t = a;
    return r;
  }
};
template <class T> int Arr<T>::size() const { return sizeof(T); }
template <class T> int Arr<T>::count = 9;
template <int N> struct Fixed {
  static constexpr int value = N;
  int f() const { return N * 2; }
};
template <class T> struct Base {
  T x;
  T bx() const { return x; }
};
template <class T> struct Derived : Base<T> {
  int y;
  // Members of the dependent base: named through `this`.
  int dy() const { return y * 2 + this->x - this->bx(); }
};
template <class T> struct W {
  T w;
};
template <class T, class U = W<T>, class V = int> struct Def {
  T a;
  U b;
  V c2;
  constexpr int sz() const { return sizeof(U) + sizeof(V); }
};
template <class T> struct Heavy {
  T get(T x) const {
    constexpr long k = fib(20);
    return x + k;
  }
};
template <class T> constexpr Box<T> boxed(T v) { return Box<T>{v}; }
template <class T> constexpr T unbox(Box<T> b) { return T(b.v) + b.get(); }
// Instantiated inside the interface: the importer's own instantiation
// of Heavy<int>::get reuses the carried value of `k`.
int heavy_one() { return Heavy<int>{}.get(1); }
constexpr int use() { return boxed(4).twice() + unbox(boxed(0)); }
} // namespace ct
