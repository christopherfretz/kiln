// [class.bit]/1, [dcl.spec.general]/3: a bit-field whose decl-specifier is an
// undeclared name is "unknown type name", then an anonymous bit-field
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:3 error "unknown type name 'y'"
//@ diag: 16:3 error "unknown type name 'y1'"
//@ diag: 18:3 error "unknown type name 'y2'"
//@ diag: 18:10 error "anonymous bit-field cannot have a default member initializer"
//@ diag: 19:3 error "unknown type name 'y3'"
//@ diag: 19:10 error "anonymous bit-field cannot have a default member initializer"
//@ diag: 21:1 error "unknown type name 'y4'"
//@ diag: 21:4 error "expected unqualified-id"
//@ diag: 22:17 warning "expression result unused"
struct S {
  y : 3;
  y1 : 3, z1 : 4;
  int k;
  y2 : 3 = 1;
  y3 : 3 {};
};
y4 : 3;
void f() { y5 : 3; }
