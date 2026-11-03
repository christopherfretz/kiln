// [stmt.expand]/5.2: the range of an iterating expansion statement is a
// constexpr variable, so a range that is not a constant expression (a
// function parameter) is diagnosed with the evaluator's note, and the
// statement is dropped.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 15:26 error "constexpr variable '__range' must be initialized by a constant expression"
//@ diag: 15:26 note "reference to 'a' is not a constant expression"
//@ diag: 13:28 note "declared here"
#include <array>

int sum(std::array<int, 2> a) {
  int s = 0;
  template for (auto x : a) { s += x; }
  return s;
}
