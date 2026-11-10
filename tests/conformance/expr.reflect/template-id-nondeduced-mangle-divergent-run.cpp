// [temp.over.link]/5, [temp.deduct.type]/5, [expr.reflect] P2996R13;
// spec/abi.md §8.14 "Known divergences from GCC 16.1" (WP-1260): under
// the default -fkiln-reflection-mangling=gcc, a dependent reflect-
// expression of an alias template-id keeps the alias's own arguments,
// `ta <alias> <alias args> _ <type>`, when the aliased type loses one:
// an alias that does not use its parameter (`ND<T>`, `ND = int`; g++-16
// `ta2ND_i`), or a parameter in a non-deduced context whose argument is
// not dependent, so the qualifier resolves (`DL<T, int>`, the
// `typename RC<U>::type` of `int`; `DP<const int, T>`, g++-16
// `ta2DPIiS0_E_…`, also `DP<int, T>`'s). g++-16 folds those into one
// symbol with another declaration
// (template-id-nondeduced-collide-run.cpp). Another translation unit
// calls each specialization by its symbol text (an asm label).
//@ expect: run
//@ args: -freflection
//@ unit: Inputs/template-id-nondeduced-mangle-divergent-syms.cpp
//@ skip(clang): clang does not implement P2996 reflection
//@ exit: 3
#include "Inputs/template-id-nondeduced-defs.h"

int call_by_symbol();

int main() { return call_by_symbol(); }
