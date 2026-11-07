// [expr.type.conv]/2, [dcl.init.list]/3.9-10: `R{...}` for a reference
// type `R` direct-list-initializes the reference; the cast is an lvalue
// (an xvalue for an rvalue reference) of the referenced type. A single
// element of a reference-related type binds directly; otherwise a
// temporary of the referenced type is list-initialized and bound.
//@ expect: run
//@ std: c++20
//@ stdout: 0 2.5 4 9 2 1 8
#include <cstdio>
#include <type_traits>
using CR = const double &;
using RR = int &&;
using LR = int &;
struct P { int x, y; };
using PR = const P &;
int g = 3;
double f(int a, CR b) { return a + b; }
template<class T> constexpr int t() { return T{7} + 1; }
static_assert(CR{2.5} == 2.5);
static_assert(RR{4} == 4);
static_assert(t<RR>() == 8);
static_assert(std::is_same_v<decltype((LR{g})), int &>);
static_assert(std::is_same_v<decltype((RR{4})), int &&>);
static_assert(std::is_same_v<decltype((CR{1.0})), const double &>);
int main() {
  double x = f(int{}, CR{});
  double y = CR{2.5};
  int z = RR{4};
  LR{g} = 9;
  int q = PR{1, 2}.y;
  const int *pa = &LR{g};
  std::printf("%g %g %d %d %d %d %d\n", x, y, z, g, q, pa == &g, t<RR>());
}
