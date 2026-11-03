// [expr.const]/10 (P3068R6, P3560R2): a std::meta::exception that no
// handler of the evaluation catches makes it not a constant expression;
// the note is P3068's "unhandled exception: what()" at the throwing call.
// A handler of another type does not match.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:11 note "unhandled exception: '^^int' has no identifier"
#include "kiln_meta.h"
using namespace std::meta;
consteval bool unmatched() {
  try {
    (void)identifier_of(^^int);
  } catch (int) {
    return true;
  }
  return false;
}
static_assert(unmatched());

int main() {}
