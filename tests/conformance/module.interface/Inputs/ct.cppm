// Interface unit of module `ct` for the carried-instantiation tests
// (spec/modules.md §11.11 Part 2, WP-1046): templates whose constant
// evaluations the unit itself instantiates, so a kind-2 module file
// carries their results.
export module ct;
constexpr long fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
export template <int N> struct Fib {
  static constexpr long value = fib(N);
  static_assert(value >= 0, "fib is never negative");
  static constexpr bool big = value > 1000;
};
export template <int N> constexpr long twice() {
  constexpr long v = fib(N) * 2;
  if constexpr (v > 100)
    return v;
  else
    return -v;
}
export template <class T, int N> struct Arr {
  T data[Fib<N>::value % 7 + 1];
  static constexpr int size = Fib<N>::value % 7 + 1;
  constexpr int first() const { return size > 2 ? 1 : 0; }
};
export template <int N> constexpr long vt = fib(N) + 1;
export template <long V> struct Tag {
  static constexpr long value = V;
};
export template <int N> using FibTag = Tag<Fib<N>::value>;
export constexpr long use = Fib<20>::value + Fib<18>::value + twice<17>() + Arr<int, 19>::size;
export constexpr long use2 = vt<16> + twice<3>() + FibTag<15>::value;
export constexpr bool big20 = Fib<20>::big;
