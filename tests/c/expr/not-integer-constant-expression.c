// C17 6.6p6, 6.7.2.1p4, 6.7.2.2p2, 6.8.4.2p3: an enumerator value, a
// bit-field width and a case label are integer constant expressions;
// clang's `VerifyIntegerConstantExpression` says "integer" in C (C++:
// "integral") and names a non-integer type (WP-1312, WP-1296 follow-up).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:12 error "expression is not an integer constant expression"
//@ diag: 14:12 error "integer constant expression must have integer type, not 'double'"
//@ diag: 15:20 error "expression is not an integer constant expression"
//@ diag: 18:8 error "expression is not an integer constant expression"
//@ diag: 21:14 error "expression is not an integer constant expression"
int n;
enum { X = n };
enum { Y = 1.0 };
struct S { int b : n; };
void f(int m) {
  switch (m) {
  case n:
    break;
  }
  enum { Z = sizeof(int[m]) };
}
