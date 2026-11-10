// [temp.over.link]/5, [expr.reflect] P2996R13, Itanium ABI
// <template-arg>; spec/abi.md §8.14, spec/reflection.md §8.3 (WP-1250): a
// dependent reflect-expression of an alias or variable template-id in a
// signature mangles as g++-16 writes it under the default
// -fkiln-reflection-mangling=gcc: `ta <alias> <args of the aliased class
// specialization> _ <type>` (`^^AT<T>`, an alias whose parameters the
// aliased type determines) and `de <unresolved-name>` (`^^vt<T>`), in a
// decltype return type, a parameter type and a requires-clause
// (Inputs/template-id-signature-defs.h; the symbols that diverge from
// g++-16's are in template-id-signature-mangle-divergent-run.cpp). Another
// translation unit calls each specialization by its symbol text (an asm
// label), so the program links only if the spelling is exactly that.
// The harness runs no `unit:` test under GCC; the driver test
// reflection_mangling.rs `gcc_dependent_template_id_link` links a kiln
// object against a g++-16 one for these (spec/landed/abi/WP-1250.md).
// Before WP-1250 kiln wrote `^^AT<T>` as the aliased type (`ty2TTIPS0_E`),
// the same symbol as `^^TT<T*>`, so the two `p_ta` overloads collided.
//@ expect: run
//@ args: -freflection
//@ unit: Inputs/template-id-signature-mangle-gcc-syms.cpp
//@ skip(clang): clang does not implement P2996 reflection
//@ exit: 11
#include "Inputs/template-id-signature-defs.h"

template <class T> int p_ta(T, K<^^AT<T>>) { return 7; }
template <class T> int p_ta(T, K<^^TT<T*>>) { return 0; }
template int p_ta(int, K<^^AT<int>>);
template int p_ta(int, K<^^TT<int*>>);

int call_by_symbol();

int main() { return call_by_symbol(); }
