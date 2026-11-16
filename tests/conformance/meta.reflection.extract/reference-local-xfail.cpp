// [meta.reflection.extract] (extract-ref, extract-value): a reference
// variable whose lifetime began within the evaluation (a reference
// parameter or local of a consteval function, [basic.life]/1) extracts
// to the object it refers to. GCC 16.1 rejects it too, so no GCC oracle
// (spec/reflection-queries.md §5). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(kiln): WP-1505: a reflected reference local or parameter has no memory slot (Oven::meta_local), so extract cannot reach its referent
#include <meta>

consteval int by_param(int &r) { return std::meta::extract<int>(^^r); }
consteval int call_param() { int x = 3; return by_param(x); }
static_assert(call_param() == 3);

consteval int by_local() {
  int x = 5;
  int &q = x;
  std::meta::extract<int &>(^^q) = 6;
  return x;
}
static_assert(by_local() == 6);
