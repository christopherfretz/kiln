// [temp.dep.expr] P2996R13: a class member access whose splice operand is
// value-dependent, or whose object expression is type-dependent, is
// resolved at instantiation: a data member through `.`, a member
// function through `->`. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 8 16 9
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
struct S {
  int m;
  int twice() const { return 2 * m; }
};
template <std::meta::info R, class T> int get(const T& t) { return t.[: R :]; }
template <std::meta::info R, class T> int call(const T* t) { return t->[: R :](); }
template <class T> int dep_object(T t) { return t.[: ^^S::m :] + 1; }

int main() {
  S s{8};
  printf("%d %d %d\n", get<^^S::m>(s), call<^^S::twice>(&s), dep_object(s));
  return 0;
}
