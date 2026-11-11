export module imge;
export namespace ie {
using nullptr_t = decltype(nullptr);
template <class T, T v> struct ic {
  static constexpr T value = v;
  constexpr operator T() const { return v; }
};
template <bool B> using bc = ic<bool, B>;
template <class T> struct is_ptr : ic<bool, false> {};
template <class T> struct is_ptr<T *> : ic<bool, true> {};
template <class T> using not_ptr = bc<!is_ptr<T>::value>;
template <int N> using twice = ic<int, N * 2>;
using three = ic<int, 1 + 2>;
template <int N> struct B { static constexpr int v = N; };
inline int g() { return B<1 + 1>::v; }
template <class T> T val();
template <class T, class U> decltype(T() + U()) add(T t, U u) { return t + u; }
template <class T> using minus1 = decltype(T() - 1);
template <class T> int make() noexcept(noexcept(T())) { return (void)T(), 2; }
template <class T> struct pair {
  T a;
  void swap(pair &o) noexcept(noexcept(a = o.a)) {
    T t = a;
    a = o.a;
    o.a = t;
  }
};
template <class T> void swap(pair<T> &x, pair<T> &y) noexcept(noexcept(x.swap(y))) { x.swap(y); }
template <class T> struct buf { T data[sizeof(T) + 1]; };
template <class... Ts> struct list { static constexpr int n = sizeof...(Ts); };
template <class... Ts> using dup = list<Ts..., Ts...>;
}
