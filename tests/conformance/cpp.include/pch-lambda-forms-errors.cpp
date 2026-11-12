// [class.local], [expr.prim.lambda.closure]: a header's local class is
// not visible outside its function, and a closure type with a capture
// has a deleted copy assignment operator, through a precompiled header
// (WP-1350).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:20 error "no member named 'P' in namespace 'lf'"
//@ diag: 13:15 error "cannot be assigned because its copy assignment operator is implicitly deleted"
#include "Inputs/lambda_forms.h"

int main() {
  (void)sizeof(lf::P);
  lf::withcap = lf::withcap;
  return lf::plain(1);
}
