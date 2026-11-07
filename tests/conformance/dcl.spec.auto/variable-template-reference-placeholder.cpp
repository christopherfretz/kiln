// [dcl.spec.auto.general]/7, [temp.inst]/7: a variable template declared
// with `auto&` (or `auto*`) has a placeholder type; each specialization
// deduces it from its instantiated initializer, so a use of the
// specialization, including inside another constexpr variable template's
// initializer, sees the deduced type rather than "depends on itself". The
// shape of folly's `constexpr_math` constants. WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 2 6 8
#include <cstdio>
template <class T> struct S { T v; constexpr T get() const { return v; } };
template <class B> inline constexpr auto vv = S<typename B::value_type>{B::value};
template <class T> struct ic { using value_type = T; static constexpr T value = 2; };
template <class T> constexpr auto& v2 = vv<ic<T>>;
template <class T> constexpr const auto* p2 = &vv<ic<T>>;
template <class T> constexpr T f(T x) { constexpr auto& r = v2<T>; return x * r.get(); }
int main() {
  constexpr auto& r = v2<float>;
  static_assert(r.get() == 2.0f);
  static_assert(p2<int>->get() == 2);
  std::printf("%d %d %d\n", (int)r.get(), (int)f(3.0f), p2<int>->get() * 4);
}
