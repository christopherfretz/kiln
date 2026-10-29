// [expr.ass]/1 via clang CheckForModifiableLvalue: arrays and functions
// are lvalues that cannot be assigned or incremented.
//@ expect: compile-fail
//@ diag: 11:5 error "array type 'int[2]' is not assignable"
//@ diag: 12:5 error "non-object type 'int ()' is not assignable"
//@ diag: 13:3 error "cannot increment value of type 'int ()'"
//@ diag: 14:5 error "invalid operands to binary expression ('int[2]' and 'int')"
int f();
void g() {
  int a[2];
  a = a;
  f = f;
  ++f;
  a += 1;
}
