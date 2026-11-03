// [expr.type.conv]/2, [dcl.init.list]/3.9: a braced functional cast
// whose list has one element (`int{6.5}`, `T{4.5}` of a one-member
// aggregate, through a typedef or parentheses) reports only the narrowing
// error: clang 22 does not check that element's implicit conversion for
// -Wliteral-conversion / -Wconstant-conversion. A list of several
// elements, or a nested list, still gets both. At clang 19:
// braced-cast-literal-conversion-19.cpp. clang 22 is the oracle.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 37:11 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 37:11 note "insert an explicit cast to silence this issue"
//@ diag: 37:11 warning "implicit conversion from 'double' to 'int' changes value from 3.5 to 3"
//@ diag: 38:11 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 38:11 note "insert an explicit cast to silence this issue"
//@ diag: 39:12 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 39:12 note "insert an explicit cast to silence this issue"
//@ diag: 39:12 warning "implicit conversion from 'double' to 'int' changes value from 5.5 to 5"
//@ diag: 40:13 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 40:13 note "insert an explicit cast to silence this issue"
//@ diag: 41:11 error "type 'double' cannot be narrowed to 'I' (aka 'int') in initializer list"
//@ diag: 41:11 note "insert an explicit cast to silence this issue"
//@ diag: 42:20 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 42:20 note "insert an explicit cast to silence this issue"
//@ diag: 43:13 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 43:13 note "insert an explicit cast to silence this issue"
//@ diag: 44:15 error "constant expression evaluates to 70000 which cannot be narrowed to type 'short'"
//@ diag: 44:15 note "insert an explicit cast to silence this issue"
//@ diag: 45:12 error "constant expression evaluates to 1000 which cannot be narrowed to type 'SC' (aka 'signed char')"
//@ diag: 45:12 note "insert an explicit cast to silence this issue"
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
