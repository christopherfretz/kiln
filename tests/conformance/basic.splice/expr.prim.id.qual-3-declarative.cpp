// [expr.prim.id.qual]/3 P2996R13: a declarative nested-name-specifier
// (of a declarator-id, a class-head-name or an enum-head-name) shall not
// have a splice-scope-specifier, so a splice cannot name the scope of an
// out-of-line definition or of a class or enum definition.
// kiln is IS-strict; GCC 16.1 accepts these declarations.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 27:5 error "a splice-scope-specifier cannot be used in a declarative nested-name-specifier"
//@ diag: 28:6 error "a splice-scope-specifier cannot be used in a declarative nested-name-specifier"
//@ diag: 29:5 error "a splice-scope-specifier cannot be used in a declarative nested-name-specifier"
//@ diag: 30:6 error "a splice-scope-specifier cannot be used in a declarative nested-name-specifier"
//@ diag: 31:8 error "a splice-scope-specifier cannot be used in a declarative nested-name-specifier"
//@ diag: 32:12 error "a splice-scope-specifier cannot be used in a declarative nested-name-specifier"
#include "kiln_meta.h"
namespace N {
extern int x;
void f();
struct C {
  static int s;
  void g();
};
struct D;
enum class E : int;
}  // namespace N
int [: ^^N :]::x = 1;
void [: ^^N :]::f() {}
int [: ^^N::C :]::s = 2;
void [: ^^N::C :]::g() {}
struct [: ^^N :]::D {};
enum class [: ^^N :]::E : int { A };
