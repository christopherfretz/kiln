// __builtin_elementwise_add_sat / __builtin_elementwise_sub_sat (clang
// LanguageExtensions "Vector builtins"; constant-evaluated from clang 20):
// saturating integer add/sub on a scalar or vector operand of one integer
// type, the operand type kept. libc++ 22's <numeric> std::add_sat /
// std::sub_sat ([numeric.sat.func]) call them. kiln does not declare them
// ("use of undeclared identifier"): the M5 cluster "saturation builtins"
// (docs/status/libcxx-m5.md: add_sat.pass, sub_sat.pass,
// numerics nodiscard.verify).
//@ requires: clang>=22
//@ expect: run
//@ stdout: 2147483647 0
#include <stdio.h>
constexpr signed char a = __builtin_elementwise_add_sat((signed char)100, (signed char)100);
constexpr unsigned b = __builtin_elementwise_sub_sat(3u, 5u);
static_assert(a == 127 && b == 0);
int main(int argc, char**) {
  int x = __builtin_elementwise_add_sat(2147483647, argc);
  unsigned char y = __builtin_elementwise_sub_sat((unsigned char)argc, (unsigned char)9);
  printf("%d %d\n", x, (int)y);
}
