// [expr.reflect]/7.1.3: `^^x` where `x` names a local parameter of a
// requires-expression is ill-formed. A parameter of the enclosing
// function named inside the requirement body is not one, and stays
// reflectable. Found running GCC's C++ testsuite; GCC 16.1 agrees.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 20:29 error "'^^' cannot be applied to a local parameter of a requires-expression 'a'"
//@ diag: 23:37 error "'^^' cannot be applied to a local parameter of a requires-expression 'q'"
#include <meta>

template <class T>
concept C = requires(T t) { ^^T; };
static_assert(C<int>);

void f(int p) {
  (void)requires(int a) { ^^p; };
  (void)requires(int a) { ^^a; };
}
template <class T>
bool g() { return requires(T q) { ^^q; }; }
