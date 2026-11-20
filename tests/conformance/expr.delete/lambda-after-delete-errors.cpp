// [expr.delete]/1: `delete []` followed by a lambda is still an array
// delete. clang reports "'[]' after delete interpreted as 'delete[]'"
// at `delete`, parses the lambda with its postfix suffix, and recovers
// by deleting its result as an object; `(int a, ...` is no lambda
// lookahead, so it is the array form's operand (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:3 error "'[]' after delete interpreted as 'delete[]'; add parentheses to treat this as a lambda-expression"
//@ diag: 18:3 error "'[]' after delete interpreted as 'delete[]'; add parentheses to treat this as a lambda-expression"
//@ diag: 19:3 error "'[]' after delete interpreted as 'delete[]'; add parentheses to treat this as a lambda-expression"
//@ diag: 20:17 error "expected ')'"
//@ diag: 20:12 note "to match this '('"
//@ diag: 20:29 error "expected expression"

struct S {};
void f(S* p) {
  delete [] { return (S*)nullptr; }();
  delete []() { return (S*)nullptr; }();
  delete [] <class T>(T t) { return t; }(p);
  delete [](int a, int b) { return (S*)nullptr; }(1, 2);
}
