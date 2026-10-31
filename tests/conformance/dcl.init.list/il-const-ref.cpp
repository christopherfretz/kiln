// [dcl.init.list]/3.10, [class.temporary]/6: a const reference to
// std::initializer_list binds a temporary built from the list; it and its
// backing array live as long as the reference.
#include "Inputs/initializer_list.h"
constexpr int f() {
  const std::initializer_list<int>& r = {10, 20, 30};
  return int(r.size()) + r.begin()[2];
}
static_assert(f() == 33);
constexpr int g() {
  std::initializer_list<int>&& r = {1, 2};
  return int(r.size());
}
static_assert(g() == 2);
