// [meta.reflection.extract]: extract of a variable whose lifetime began
// within the constant evaluation (a local of a consteval function, or
// its parameter) reads that variable. Found running GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
//@ xfail(kiln): WP-1482, a reflected automatic variable has no frame to read; only static storage is extracted
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
