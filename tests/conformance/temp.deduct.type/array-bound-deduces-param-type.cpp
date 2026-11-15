// [temp.deduct.type]/20 (its example): a non-type parameter `i` of type
// `T` deduced from an array bound deduces `T` as `std::size_t`, unless
// another argument deduces `T`, before or after the bound. Found running
// GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++17
using size_t = decltype(sizeof 0);
template <class T, T n> constexpr T h(int (&)[n]) { return n; }
template <class T, T n> constexpr T f(T, int (&)[n]) { return n; }
template <class T, T n> constexpr T g(int (&)[n], T) { return n; }
template <class T, T n> constexpr T k(int (&)[n], T = T()) { return n; }

int ar[42];
static_assert(__is_same(decltype(h(ar)), size_t));
static_assert(h(ar) == 42);
static_assert(__is_same(decltype(f(1, ar)), int));
static_assert(__is_same(decltype(g(ar, 1L)), long));
static_assert(g(ar, 1L) == 42);
static_assert(__is_same(decltype(k(ar)), size_t));

template <class U> void in_tmpl(U u) {
  int a[3];
  static_assert(__is_same(decltype(h(a)), size_t));
  static_assert(__is_same(decltype(g(a, u)), U));
}
template void in_tmpl<short>(short);
