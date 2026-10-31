// [dcl.type.auto.deduct]/4: the elements of the list must deduce the
// same type; a direct-list-initialization takes exactly one element.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "deduced conflicting types ('int' vs 'double') for initializer list element type"
//@ diag: 11:13 error "initializer for variable 'b' with type 'auto' contains multiple expressions"
//@ diag: 12:8 error "cannot deduce actual type for variable 'c' with type 'auto' from initializer list"
#include "../dcl.init.list/Inputs/initializer_list.h"
void f() {
  auto a = {1, 2.0};
  auto b{1, 2};
  auto c = {};
}
