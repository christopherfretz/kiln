// The accepted side of p13-special-members-by-std.cpp: from C++20 a
// closure type without a lambda-capture is default constructible and
// assignable; any closure type is copy and move constructible.
//@ expect: compile-ok
//@ args: -std=c++20
#include <type_traits>
void f(int i) {
  auto nc = [] { return 1; };
  decltype(nc) nc2;
  nc2 = nc;
  nc2 = static_cast<decltype(nc) &&>(nc);
  auto lam = [i] {};
  decltype(lam) copy = lam;
  decltype(lam) moved = static_cast<decltype(lam) &&>(copy);
  static_assert(std::is_default_constructible_v<decltype(nc)>);
  static_assert(std::is_copy_assignable_v<decltype(nc)>);
  static_assert(!std::is_default_constructible_v<decltype(lam)>);
  static_assert(!std::is_copy_assignable_v<decltype(lam)>);
  static_assert(!std::is_move_assignable_v<decltype(lam)>);
  (void)moved;
}
