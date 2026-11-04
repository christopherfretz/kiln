// [dcl.struct.bind] of a static constexpr array of std::meta::info: the
// decomposed variable has a consteval-only type (P2996R13
// [basic.types.general]), and each binding's expression (e[i]) is
// checked as part of that variable's initializer rather than as a
// full-expression of its own, so it is not "a consteval-only expression
// outside a constant-evaluated context" (spec/reflection-repr.md §6).
// Neither the array nor the decomposition is emitted (spec/codegen.md
// "Consteval-only variables"; WP-534). GCC 16.1 compiles and runs it.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
static constexpr std::meta::info arr[] = {^^int, ^^long};
int f() {
  static constexpr auto [a, b] = arr;
  return std::meta::is_type(a) + std::meta::is_type(b);
}
int main() { return f() - 2; }
