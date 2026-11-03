// [expr.prim.splice]/2 P2996R13: in an expression position a splice must
// designate a variable, function, enumerator, structured binding, value or
// object (a type needs `typename`, a template `template`); a constructor
// or destructor is ill-formed in any splice-expression, the member access
// form included.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 27:12 error "splice in an expression position does not designate a variable, function, enumerator, value or object"
//@ diag: 28:12 error "splice in an expression position does not designate a variable, function, enumerator, value or object"
//@ diag: 30:6 error "a splice-expression cannot designate a constructor"
//@ diag: 31:8 error "a splice-expression cannot designate a destructor"
//@ diag: 32:13 error "a splice-expression cannot designate a destructor"
#include "kiln_meta.h"
namespace N {}
struct T {
  T();
  ~T();
};
consteval std::meta::info member(const char* n) {
  for (unsigned long i = 0; i < std::meta::__kiln_members_count(^^T); ++i)
    if (std::meta::name_is(std::meta::__kiln_member_at(^^T, i), n))
      return std::meta::__kiln_member_at(^^T, i);
  return ^^int;
}
int a = [: ^^int :];
int b = [: ^^N :];
void f(T& t) {
  [: member("T") :];
  t.[: member("~T") :]();
  (void)&[: member("~T") :];
}
