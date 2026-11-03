// [dcl.attr.indet]/1 (C++26, P2795R5): `[[indeterminate]]` applies to the
// definition of a block variable with automatic storage duration or to a
// parameter of a function declaration; anywhere else it is ill-formed
// (clang trunk's wording: "local variables or function parameters").
// [dcl.attr.grammar]/4: it takes no argument clause.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 16:9 error "'indeterminate' attribute only applies to local variables or function parameters"
//@ diag: 18:11 error "'indeterminate' attribute only applies to local variables or function parameters"
//@ diag: 20:3 error "'indeterminate' attribute only applies to local variables or function parameters"
//@ diag: 22:18 error "'indeterminate' attribute only applies to local variables or function parameters"
//@ diag: 23:24 error "'indeterminate' attribute only applies to local variables or function parameters"
//@ diag: 24:18 error "'indeterminate' attribute only applies to local variables or function parameters"
//@ diag: 25:24 error "attribute 'indeterminate' cannot have an argument list"
int g [[indeterminate]];
struct S {
  int m [[indeterminate]];
};
[[indeterminate]] void fn();
void f() {
  static int s [[indeterminate]];
  thread_local int t [[indeterminate]];
  extern int e [[indeterminate]];
  int a [[indeterminate()]];
  int ok [[indeterminate]];
  (void)ok;
}
