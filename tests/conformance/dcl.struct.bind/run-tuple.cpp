//@ expect: run
//@ stdout: 3 4.5 7 8 7 8 | ctor ctor dtor dtor
// [dcl.struct.bind]/4 (C++26 /7) at run time: tuple-like bindings are
// reference variables initialized from get<i>; a get returning by value
// materializes a temporary bound to the binding's reference, destroyed at
// the end of the scope in reverse order of construction.
extern "C" int printf(const char *, ...);
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct Q {
  int a;
  double b;
  template <unsigned long I> auto get() const {
    if constexpr (I == 0) return a; else return b;
  }
};
template <> struct std::tuple_size<Q> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, Q> { using type = int; };
template <> struct std::tuple_element<1, Q> { using type = double; };
namespace n {
struct R { int a, b; };
template <unsigned long I> int &get(R &r) { return I == 0 ? r.a : r.b; }
struct T {
  T() { printf(" ctor"); }
  T(const T &) { printf(" ctor"); }
  ~T() { printf(" dtor"); }
};
struct Pair { int v; };
template <unsigned long I> T get(const Pair &) { return T(); }
}
template <> struct std::tuple_size<n::R> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, n::R> { using type = int &; };
template <> struct std::tuple_size<n::Pair> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, n::Pair> { using type = n::T; };
int main() {
  auto [m, k] = Q{3, 4.5};
  n::R r{1, 2};
  auto &&[p, q] = r;
  p = 7;
  auto &[s, t] = r;
  t = 8;
  printf("%d %g %d %d %d %d |", m, k, p, q, r.a, r.b);
  {
    auto [u, v] = n::Pair{0};
  }
  printf("\n");
}
