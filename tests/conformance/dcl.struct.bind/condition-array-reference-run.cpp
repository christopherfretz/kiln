// [stmt.pre]/4-5, [dcl.struct.bind]/1, /4 (C++26, P0963R3): a structured
// binding declaration as a condition introduces the variable `e`, declared
// as `S cv A e` with the ref-qualifier of the declaration; the condition's
// requirement that the declarator not specify an array is read of that
// declarator, so `auto& [a, b] = arr` (a reference to an array) is
// allowed, and the decision variable `e`, an lvalue of array type,
// converts to `true`. `auto [a, b] = arr` is rejected
// (condition-array-errors.cpp). clang 22 agrees; GCC 16.1 rejects every
// array initializer in a condition (a recorded decision,
// docs/status/deviations.md "Structured binding of an array in a
// condition"). Found running GCC's C++ testsuite.
//@ std: c++26
//@ expect: run
//@ requires: clang>=22
int arr[2] = {1, 2};
int main() {
  int n = 0;
  if (auto& [a, b] = arr) n += a + b;
  if (const auto& [a, b] = arr) n += a * b;
  for (int i = 0; auto&& [a, b] = arr; ++i) {
    if (i == 2) break;
    n += b;
  }
  while (auto& [a, b] = arr) {
    a = 5;
    break;
  }
  return n == 9 && arr[0] == 5 ? 0 : 1;
}
