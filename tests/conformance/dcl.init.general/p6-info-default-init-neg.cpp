// [dcl.init.general]/6.3-6.4: only the `std::meta::info` subobjects of a
// default-initialized object are zero-initialized; its other scalar
// members stay indeterminate ([basic.indet]), so reading one is not a
// constant expression ([expr.const]/10.9).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:27 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 23:15 note "in call to 'read_int()'"
#include "kiln_meta.h"
using std::meta::info;

struct S {
  info m;
  int i;
};
consteval bool read_int() {
  S s;
  return s.m == info{} && s.i == 0;
}
static_assert(read_int());
