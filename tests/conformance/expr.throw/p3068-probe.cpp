// [expr.const]/10 (C++26, P3068R6): a throw-expression whose exception is
// caught within the same constant evaluation is permitted.
// constexpr-throw.cpp pins the pre-P3068 (clang 19) rejection.
// WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ std: c++26
constexpr int f() {
  try {
    throw 1;
  } catch (int e) {
    return e + 1;
  }
  return 0;
}
static_assert(f() == 2);
