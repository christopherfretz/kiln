// [expr.reflect]/7 (P2996R13), [class.access]/4: `^^C::f` of a private
// member function named outside C is ill-formed like any other use of the
// name. GCC 16.1 accepts it (it checks the access of a function name only
// when overload resolution picks it), so this follows the draft and is not
// GCC-compared (spec/landed/sema/WP-830.md).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:25 error "'f' is a private member of 'C'"
//@ diag: 16:8 note "implicitly declared private here"
//@ diag: 20:25 error "'g' is a private member of 'C'"
//@ diag: 17:15 note "implicitly declared private here"
#include "kiln_meta_queries.h"
class C {
  void f();
  static void g();
};
constexpr auto a = ^^C::f;
constexpr auto b = ^^C::g;
