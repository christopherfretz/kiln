// [meta.reflection.extract]: extract of a variable whose lifetime began
// within the constant evaluation (a local of a consteval function, or
// its parameter) reads that variable. Found running GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif

consteval int local() {
  int arg = 4;
  return std::meta::extract<int>(^^arg);
}
static_assert(local() == 4);

consteval int param(int arg) { return std::meta::extract<int>(^^arg); }
static_assert(param(5) == 5);

// A reference to a local.
consteval int ref_local() {
  int v = 8;
  std::meta::extract<int&>(^^v) = 9;
  return v;
}
static_assert(ref_local() == 9);

// The innermost live instance of a recursive function's local.
consteval int depth(int n) {
  int here = n;
  if (n > 0) return depth(n - 1) + std::meta::extract<int>(^^here);
  return 0;
}
static_assert(depth(3) == 6);
