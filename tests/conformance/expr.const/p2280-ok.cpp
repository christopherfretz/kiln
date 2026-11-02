// [expr.const]/5.10, /10 (C++23, P2280R4): a reference whose referent is
// unknown names "an unspecified object of the referenced type", so a
// constant expression may use it as long as the object's value is not
// needed: std::array's size(), std::size and empty() through a reference
// parameter, a constexpr member function that only uses `this` (as an
// address), and the identity of the object (comparing its address with
// itself, as clang 22 does).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ args: -stdlib=libc++
//@ requires: clang>=22
#include <array>
#include <cstddef>

template <int N> struct Widget {
  [[nodiscard]] constexpr int f() const { return N; }
  constexpr const Widget *self() const { return this; }
  constexpr bool same(const Widget &o) const { return this == &o; }
};

void arrays(std::array<int, 4> &a, const std::array<char, 2> &b) {
  static_assert(a.size() == 4);
  static_assert(b.size() == 2);
  static_assert(std::size(a) == 4);
  static_assert(!a.empty());
  static_assert(a.max_size() == 4);
  static_assert(std::tuple_size_v<std::remove_reference_t<decltype(a)>> == 4);
}

template <class C> constexpr std::size_t count(const C &c) { return c.size(); }
void forwarded(const std::array<long, 7> &a) { static_assert(count(a) == 7); }

void unknown_this(Widget<3> &w, const Widget<5> &cw) {
  static_assert(w.f() == 3);
  static_assert(cw.f() == 5);
  static_assert(w.self() == &w);
  static_assert(w.same(w));
}

void identity(int (&a)[4], int &x, std::array<int, 4> &s) {
  static_assert(&a == &a);
  static_assert(&x == &x);
  static_assert(&a[1] != &a[2]);
  static_assert(s.begin() == s.begin());
}
