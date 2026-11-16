// A call of GCC's __builtin_exit is never a constant expression
// ([expr.const]/10: it is not a constexpr function; GCC 16.1: "is not a
// constant expression"). kiln's note is the one clang gives for a call
// of __builtin_abort. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang has no __builtin_exit
//@ oracle: gcc
//@ strict-diags
//@ diag: 18:15 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 15:5 note "subexpression not valid in a constant expression"
//@ diag: 18:19 note "in call to 'f(1)'"
constexpr int f(int x) {
  if (x)
    __builtin_exit(1);
  return 0;
}
constexpr int y = f(1);
constexpr int z = f(0);
