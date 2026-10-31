// [dcl.type.auto.deduct]/2.1, [dcl.init.list]/3.6: `auto x = {...}`
// deduces std::initializer_list<U> and builds it from the list; a
// direct-list-initialization with one element deduces the element type.
#include "../dcl.init.list/Inputs/initializer_list.h"
constexpr int f() {
  auto l = {1, 2, 3};
  static_assert(__is_same(decltype(l), std::initializer_list<int>));
  int s = 0;
  for (auto p = l.begin(); p != l.end(); ++p) s += *p;
  return s + int(l.size());
}
static_assert(f() == 9);
constexpr double g() {
  auto d = {1.5};
  auto e{2.5};
  static_assert(__is_same(decltype(e), double));
  return *d.begin() + e;
}
static_assert(g() == 4.0);
template <class T> constexpr int count(T t) { return int(t.size()); }
constexpr int h() {
  const auto& r = {'a', 'b'};
  static_assert(__is_same(decltype(r), const std::initializer_list<char>&));
  return count(r);
}
static_assert(h() == 2);
