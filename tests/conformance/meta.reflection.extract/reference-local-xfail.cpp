// [meta.reflection.extract] (extract-ref, extract-value): a reference
// local whose lifetime began within the evaluation ([basic.life]/1)
// extracts to the object it refers to. GCC 16.1 rejects it too, so no GCC
// oracle (spec/reflection-queries.md §5). The reference parameter case
// passes (`reference-param.cpp`). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(kiln): WP-1515: a reference local lives in a register reused after its block, so the Oven needs its per-scope register and pc range in Func to find it
#include <meta>

consteval int by_local() {
  int x = 5;
  int &q = x;
  std::meta::extract<int &>(^^q) = 6;
  return x;
}
static_assert(by_local() == 6);
