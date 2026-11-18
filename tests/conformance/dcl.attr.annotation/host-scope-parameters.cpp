// [dcl.attr.annotation]/1: for a function parameter, the declaration X
// whose host and target scopes must agree, and which must not be a
// non-defining friend declaration, is the function declaration. So a
// parameter of a block-scope function declaration or of a non-defining
// friend declaration (also a friend function template) carries no
// annotation; one of a defining friend, of a namespace-scope function or
// of a function pointer declared in a block does. Found running GCC's
// C++ testsuite; GCC 16.1 agrees.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P3394 annotations
//@ oracle: gcc
//@ strict-diags
//@ diag: 23:19 error "an annotation cannot appertain to a parameter of a block-scope declaration whose target scope is a namespace"
//@ diag: 28:19 error "an annotation cannot appertain to a parameter of a non-defining friend declaration"
//@ diag: 29:38 error "an annotation cannot appertain to a parameter of a non-defining friend declaration"
#include <meta>

void h([[=1]] int);
static_assert(annotations_of(parameters_of(^^h)[0]).size() == 1);
void g() {
  void (*fp)([[=2]] int a) = nullptr;
  extern void h([[=3]] int);
  (void)fp;
}
class C {
  friend void k([[=4]] int a) {}
  friend void h([[=5]] int a);
  template <class T> friend void t([[=6]] T a);
  template <class T> friend void t2([[=7]] T a) {}
};
