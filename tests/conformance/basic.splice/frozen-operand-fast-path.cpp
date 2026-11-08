// [basic.splice] P2996R13, the splice fast path (spec/reflection.md §5):
// a splice whose operand is already a frozen reflection (a constexpr
// `std::meta::info` variable, a `^^` expression, a constant) reads it
// without an Oven run. 1,000 type splices, 1,000 expression splices and
// 1,000 scope splices of three frozen operands start no evaluation of
// their own: the four evaluations are the three variables' initializers
// and the static_assert.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ oven: evaluations<=4 steps<=8 splice_fast>=3000
#include "kiln_meta.h"
namespace N { int x = 2; }
int v = 1;
constexpr std::meta::info rt = ^^int;
constexpr std::meta::info rv = ^^v;
constexpr std::meta::info rn = ^^N;
#define D1(n) using T##n = [: rt :]; inline int f##n() { return [: rv :] + [: rn :]::x; }
#define D10(n) D1(n##0) D1(n##1) D1(n##2) D1(n##3) D1(n##4) D1(n##5) D1(n##6) D1(n##7) D1(n##8) D1(n##9)
#define D100(n) D10(n##0) D10(n##1) D10(n##2) D10(n##3) D10(n##4) D10(n##5) D10(n##6) D10(n##7) D10(n##8) D10(n##9)
D100(0) D100(1) D100(2) D100(3) D100(4) D100(5) D100(6) D100(7) D100(8) D100(9)
static_assert(__is_same(T999, int));
