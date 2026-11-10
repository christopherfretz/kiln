// [temp.over.link]/5, [expr.reflect] P2996R13, Itanium ABI
// <template-arg>; spec/abi.md §8.14 (WP-1250): under
// -fkiln-reflection-mangling=kiln a dependent reflect-expression of a
// template-id in a signature always keeps its template arguments and
// qualifier: an alias template-id as `ta <alias> <alias args> _ <type>`,
// a variable or function template-id as `de <unresolved-name>`, in a
// decltype return type, a parameter type and a requires-clause, after
// the vendor info type `u6__info` (a substitution candidate, so `S0_`
// in the requires-clauses). Another translation unit calls each
// specialization by its symbol text (an asm label).
//@ expect: run
//@ args: -freflection -fkiln-reflection-mangling=kiln
//@ unit: Inputs/template-id-signature-mangle-kiln-syms.cpp
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): kiln's vendor reflection mangling
//@ exit: 12
#include "Inputs/template-id-signature-defs.h"

int call_by_symbol();

int main() { return call_by_symbol(); }
