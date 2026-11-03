// [basic.splice]/1 P2996R13: the operand of a splice-specifier is a
// constant expression; the IS example, verbatim: inside a consteval
// function the parameter `v` is not a constant.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 13:12 error "splice operand must be a constant expression"
//@ diag: 13:12 note "function parameter 'v' with unknown value cannot be used in a constant expression"
//@ diag: 12:42 note "declared here"
#include "kiln_meta.h"
consteval int bad_splice(std::meta::info v) {
  return [:v:];  // error: v is not constant
}
