// [temp.over.link]/5, [expr.reflect] P2996R13, Itanium ABI
// <unresolved-name>; spec/abi.md §8.14 "Known divergences from GCC 16.1"
// (WP-1250): under the default -fkiln-reflection-mangling=gcc, a
// dependent reflect-expression of a function template-id (`^^g<T>`,
// `^^ns::operator== <T>`, `^^S<T>::template m<int>`), of a qualified
// variable template-id (`^^ns::nv<T>`) or of an alias template-id with a
// parameter its aliased type does not use (`^^F<T, int>`) in a signature
// keeps its template arguments and qualifier, `de <unresolved-name>` /
// `ta <alias> <alias args> _ <type>`, where g++-16 drops them and folds
// distinct declarations into one symbol
// (template-id-signature-collide-run.cpp); g++-16 rejects
// `^^S<T>::template m<int>` at substitution. Another translation unit
// calls each specialization by its symbol text (an asm label).
//@ expect: run
//@ args: -freflection
//@ unit: Inputs/template-id-signature-mangle-divergent-syms.cpp
//@ skip(clang): clang does not implement P2996 reflection
//@ exit: 7
#include "Inputs/template-id-signature-defs.h"

int call_by_symbol();

int main() { return call_by_symbol(); }
