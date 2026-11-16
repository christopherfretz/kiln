// [expr.prim.id.dtor]/2: the type a `~typename [: r :]` destructor name
// denotes must be the type of the object expression. GCC 16.1 rejects it
// too. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 13:15 error "destructor type 'U' in object destruction expression does not match the type 'S' of the object being destroyed"
#include <meta>
struct S { int v; };
struct U { int v; };
void f(S s) {
  s.~typename [:^^U:]();
}
