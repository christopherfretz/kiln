// [stmt.expand]/5: an expansion statement with N = 0 elements has no
// copies of its body, so a body that would be ill-formed for any element
// is never instantiated: an empty pack expansion, an empty tuple and an
// empty array (as a range) are all fine.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <array>
#include <tuple>

template <class... Ts> int pack(Ts... ts) {
  int s = 0;
  template for (auto x : {ts...}) { s += x.no_such_member(); }
  return s;
}
template <class T> void tuple_like(T t) {
  template for (auto x : t) { static_assert(sizeof(x) == 0); }
}
constexpr std::array<int, 0> none{};
template <class T> constexpr int range() {
  int s = 7;
  template for (constexpr T x : none) { s += x.bad; }
  return s;
}
int use() {
  tuple_like(std::tuple<>{});
  return pack() + range<int>();
}
static_assert(range<int>() == 7);
