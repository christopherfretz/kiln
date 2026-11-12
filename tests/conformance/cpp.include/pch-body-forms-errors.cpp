// [namespace.udecl], [dcl.spec.auto.general]: a header's block-scope
// using-declarations stay local to their function, and a deduced return
// type is deduced, through a precompiled header (WP-1343).
//@ expect: compile-fail
//@ diag: 10:9 error "use of undeclared identifier 'f'"
//@ diag: 11:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'int'"
#include "Inputs/body_forms.h"

int main() {
  (void)f(1);
  int *p = bf::cr();
  bf::D d; // see virtual_dtor_noexcept_matches_replay
  return bf::lu();
}
