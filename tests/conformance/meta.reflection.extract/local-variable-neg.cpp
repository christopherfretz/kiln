// [meta.reflection.extract], [expr.const.core]: extract of an automatic
// variable whose lifetime did not begin within the evaluation (the frame
// that held it has returned) is not a constant expression: never a read
// of a dead object.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:32 note "'^^gone' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 20:15 note "in call to 'stale()'"
//@ diag: 16:7 note "declared here"
#include "kiln_meta.h"

consteval std::meta::info leak() {
  int gone = 1;
  return ^^gone;
}
consteval int stale() { return std::meta::extract<int>(leak()); }
static_assert(stale() == 1);
