// [expr.type.conv]/2, [dcl.init.list]/3.9: at -fclang-compat=19 the one
// element of a braced functional cast is checked for -Wliteral-conversion
// and -Wconstant-conversion beside its narrowing error, as clang 19 does
// (clang 22 reports the narrowing only:
// braced-cast-literal-conversion.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 42:11 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 42:11 note "insert an explicit cast to silence this issue"
//@ diag: 42:11 warning "implicit conversion from 'double' to 'int' changes value from 3.5 to 3"
//@ diag: 43:11 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 43:11 note "insert an explicit cast to silence this issue"
//@ diag: 43:11 warning "implicit conversion from 'double' to 'int' changes value from 4.5 to 4"
//@ diag: 44:12 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 44:12 note "insert an explicit cast to silence this issue"
//@ diag: 44:12 warning "implicit conversion from 'double' to 'int' changes value from 5.5 to 5"
//@ diag: 45:13 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 45:13 note "insert an explicit cast to silence this issue"
//@ diag: 45:13 warning "implicit conversion from 'double' to 'int' changes value from 6.5 to 6"
//@ diag: 46:11 error "type 'double' cannot be narrowed to 'I' (aka 'int') in initializer list"
//@ diag: 46:11 note "insert an explicit cast to silence this issue"
//@ diag: 46:11 warning "implicit conversion from 'double' to 'I' (aka 'int') changes value from 7.5 to 7"
//@ diag: 47:20 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 47:20 note "insert an explicit cast to silence this issue"
//@ diag: 47:20 warning "implicit conversion from 'double' to 'int' changes value from 8.5 to 8"
//@ diag: 48:13 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 48:13 note "insert an explicit cast to silence this issue"
//@ diag: 48:14 warning "implicit conversion from 'double' to 'int' changes value from 9.5 to 9"
//@ diag: 49:15 error "constant expression evaluates to 70000 which cannot be narrowed to type 'short'"
//@ diag: 49:15 note "insert an explicit cast to silence this issue"
//@ diag: 50:12 error "constant expression evaluates to 1000 which cannot be narrowed to type 'SC' (aka 'signed char')"
//@ diag: 50:12 note "insert an explicit cast to silence this issue"
//@ diag: 49:15 warning "implicit conversion from 'int' to 'short' changes value from 70000 to 4464"
//@ diag: 50:12 warning "implicit conversion from 'int' to 'SC' (aka 'signed char') changes value from 1000 to -24"
struct S { int m; char c; };
struct T { int m; };
struct U { S s; };
using I = int;
using SC = signed char;
void g() {
  (void)S{3.5, 'x'};
  (void)T{4.5};
  (void)U{{5.5, 'y'}};
  (void)int{6.5};
  (void)I{7.5};
  int arr[] = {int{8.5}};
  (void)int{(9.5)};
  (void)short{70000};
  (void)SC{1000};
}
