// [dcl.type.auto.deduct]/2.1, [temp.deduct.call]/1: deducing `auto` from a
// copy-list-initializer deduces `std::initializer_list<U>` from each
// element; a nested braced-init-list element is a non-deduced context and
// is converted to the `U` the other elements deduce (libc++'s pstl tests:
// `for (const auto& p : {std::pair{0, 34}, {1, 36}})`).
//@ strict-diags
#include <initializer_list>
template <class A, class B> struct P { A a; B b; };
template <class A, class B> P(A, B) -> P<A, B>;
constexpr int sum() {
  int s = 0;
  for (const auto &p : {P{0, 34}, {1, 36}, {2, 39}})
    s += p.a + p.b;
  return s;
}
static_assert(sum() == 112);
template <class T> constexpr T tsum() {
  T s = 0;
  for (const auto &p : {P<T, T>{1, 2}, {3, 4}})
    s += p.a * p.b;
  return s;
}
static_assert(tsum<long>() == 14);
constexpr auto l = {P{1, 'a'}, {2, 'b'}, P{3, 'c'}};
static_assert(__is_same(decltype(l), const std::initializer_list<P<int, char>>));
static_assert(l.size() == 3 && l.begin()[1].b == 'b');
