//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:12 error "redefinition of 'z'"
//@ diag: 11:9 note "previous definition is here"
//@ diag: 13:9 error "redefinition of 'w'"
//@ diag: 12:7 note "previous definition is here"
// [basic.scope.block]/2, [dcl.struct.bind]/1: bindings are declared in the
// enclosing scope, so a repeated name is a redefinition.
struct S { int a, b; };
void f(S s) {
  auto [z, z] = s;
  int w = 0;
  auto [w, v] = s;
}
