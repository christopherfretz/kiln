// [stmt.expand]: expansion statements nest; the inner statement is
// expanded once per element of the outer one (each copy sees that outer
// element), across the three forms, and an inner pattern that depends on
// the outer constexpr element is expanded with it.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <array>
#include <tuple>

constexpr int lists() {
  int s = 0;
  template for (auto x : {1, 2}) {
    template for (auto y : {10, 20}) { s += x * y; }
  }
  return s;
}
static_assert(lists() == 90);

constexpr std::array<int, 2> arr{3, 4};
constexpr int mixed() {
  int s = 0;
  template for (constexpr int x : arr) {
    template for (auto y : std::tuple<int, long>{x, x * 2}) { s += y; }
  }
  return s;
}
static_assert(mixed() == 3 + 6 + 4 + 8);


constexpr int dependent_inner() {
  int s = 0;
  template for (constexpr int n : {1, 2, 3}) {
    template for (auto y : std::array<int, n>{}) { s += y + 1; }
  }
  return s;
}
static_assert(dependent_inner() == 6);
