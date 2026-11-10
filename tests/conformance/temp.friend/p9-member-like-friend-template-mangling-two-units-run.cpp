// [temp.friend]/9, Itanium ABI <unqualified-name> ::= F <source-name>
// (clang `isMemberLikeConstrainedFriend`): a friend template whose
// constraint names a parameter of the enclosing class template declares
// a template of its own in each specialization, mangled with `F`; clang
// 22 gives it the class's prefix (`_ZN1AIiEF2g2...`). A specialization
// instantiated from a friend declaration with a trailing requires-clause
// takes a nested name even at namespace scope (`_ZN2n1IiEE...`).
// Inputs/p9-member-like-friend-template-mangling-defs.cpp calls each
// specialization under clang 22's name (an asm label), so the program
// links only if kiln defines the same names (spec/abi.md, WP-1146).
//@ requires: clang>=22
//@ expect: run
//@ args: %{test_dir}/Inputs/p9-member-like-friend-template-mangling-defs.cpp
//@ stdout: 66 66
#include "Inputs/p9-member-like-friend-templates.h"
#include <cstdio>
int other();
int main() {
  S *s = nullptr;
  A<int> *a = nullptr;
  std::printf("%d %d\n", n1(0, s) + n2(0, s) + n3(0) + g1(0, a) + g2(0, a) + g3(0, a), other());
}
