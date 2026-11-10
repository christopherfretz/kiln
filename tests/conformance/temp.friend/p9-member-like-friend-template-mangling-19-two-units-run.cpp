// [temp.friend]/9, Itanium ABI <unqualified-name> ::= F <source-name>,
// at -fclang-compat=19 (the clang 22 form is
// p9-member-like-friend-template-mangling-two-units-run.cpp): clang 19
// gives a friend template constrained on the enclosing class's `T` no
// class prefix (`_ZNF2g2...`), and a template declared at namespace scope
// first but defined as a constrained friend a nested name (`_ZN2n3...`).
// Inputs/p9-member-like-friend-template-mangling-defs-19.cpp calls each
// specialization under clang 19's name (an asm label), so the program
// links only if kiln defines the same names (spec/abi.md, WP-1146).
//@ requires: clang<22
//@ expect: run
//@ args: %{test_dir}/Inputs/p9-member-like-friend-template-mangling-defs-19.cpp
//@ stdout: 66 66
#include "Inputs/p9-member-like-friend-templates.h"
#include <cstdio>
int other();
int main() {
  S *s = nullptr;
  A<int> *a = nullptr;
  std::printf("%d %d\n", n1(0, s) + n2(0, s) + n3(0) + g1(0, a) + g2(0, a) + g3(0, a), other());
}
