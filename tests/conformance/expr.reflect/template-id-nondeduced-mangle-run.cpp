// [temp.over.link]/5, [temp.deduct.type]/5, [expr.reflect] P2996R13,
// Itanium ABI <template-arg>; spec/abi.md §8.14, spec/reflection.md §8.3
// (WP-1260): a dependent reflect-expression of an alias template-id whose
// parameter appears only in a non-deduced context of the defining-type-id
// (a nested-name-specifier: `D = typename RC<T>::type`; a decltype:
// `DT = decltype(T{})`) mangles as g++-16 writes it under the default
// -fkiln-reflection-mangling=gcc when that argument is dependent:
// `ta1D_N2RCIS0_E4typeE`, the aliased type keeping the dependent
// qualifier, which determines the alias's arguments. Also partly
// dependent arguments (`D2<T, int>`, `DP<T, int>`, `DP<int, T>`), nested
// aliases (`D<AT<T>>`, `AT<D<T>>`, `D<D<T>>`, `TT<D<T>>`, `D<T>*`), a
// pack expansion (`TT<typename RC<T>::type...>`), a non-type parameter
// (`DN<N>`) and a decltype (`DT<T>`, `NDS<T>`), in a decltype return type,
// a parameter type and a requires-clause
// (Inputs/template-id-nondeduced-defs.h; the symbols that keep kiln's
// spelling are in template-id-nondeduced-mangle-divergent-run.cpp).
// Another translation unit calls each specialization by its symbol text
// (an asm label), so the program links only if the spelling is exactly
// that. The driver test reflection_mangling.rs
// `gcc_nondeduced_alias_link` links a kiln object against a g++-16 one
// for these. Before WP-1260 kiln wrote the alias's own arguments,
// `ta1DIS0_E_N2RCIS0_E4typeE`.
//@ expect: run
//@ args: -freflection
//@ unit: Inputs/template-id-nondeduced-gcc-syms.cpp
//@ skip(clang): clang does not implement P2996 reflection
//@ exit: 16
#include "Inputs/template-id-nondeduced-defs.h"

int call_by_symbol();

int main() { return call_by_symbol(); }
