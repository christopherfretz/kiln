// [expr.prim.lambda.general]/4: a static lambda has no implicit object
// parameter, so `this` cannot be used in it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:33 error "invalid use of 'this' outside of a non-static member function"
void f() {
  auto i = []() static { return this; };
}
