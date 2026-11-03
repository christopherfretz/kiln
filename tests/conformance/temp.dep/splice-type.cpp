// [temp.dep.type] P2996R13: a splice-type-specifier whose operand is
// value-dependent is a dependent type, substituted at instantiation, in a
// typename-specifier, an alias template, a return type and a member
// declaration. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 6 2 8 1
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
template <std::meta::info R> using TT = [: R :];
template <std::meta::info R> typename [: R :] make() { return 6; }
template <std::meta::info R> struct Box { [: R :] v; };
template <class T> int size_of() { return sizeof(typename [: ^^T :]); }

int main() {
  TT<^^int> v = 2;
  Box<^^long> b{8};
  printf("%d %d %d %d\n", make<^^int>(), v, static_cast<int>(b.v), size_of<char>());
  return 0;
}
