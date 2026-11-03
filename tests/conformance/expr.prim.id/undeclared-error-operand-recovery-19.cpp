// [expr.prim.id.unqual]/1 (recovery): undeclared-error-operand-recovery-22.cpp's
// forms under clang 19, where an undeclared plain identifier is a delayed
// typo; a parenthesized expression beginning with one (not followed by
// '(', '<' or '::') is an error expression whose tokens are skipped to
// its ')'.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 50:5 error "use of undeclared identifier 'u'"
//@ diag: 51:8 error "use of undeclared identifier 'u'"
//@ diag: 52:7 error "expected ']'"
//@ diag: 52:4 note "to match this '['"
//@ diag: 53:7 error "expected ']'"
//@ diag: 53:4 note "to match this '['"
//@ diag: 54:7 error "expected ']'"
//@ diag: 54:4 note "to match this '['"
//@ diag: 54:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 55:4 error "use of undeclared identifier 'u'"
//@ diag: 55:14 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 56:4 error "use of undeclared identifier 'u'"
//@ diag: 56:12 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 57:5 error "use of undeclared identifier 'u'"
//@ diag: 58:4 error "use of undeclared identifier 'u'"
//@ diag: 59:10 error "expected ')'"
//@ diag: 59:3 note "to match this '('"
//@ diag: 59:8 error "use of undeclared identifier 'u'"
//@ diag: 60:4 error "use of undeclared identifier 'u'"
//@ diag: 61:8 error "expected expression"
//@ diag: 61:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 62:12 error "use of undeclared identifier 'u'"
//@ diag: 63:12 error "use of undeclared identifier 'u'"
//@ diag: 63:22 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 64:6 error "use of undeclared identifier 'u'"
//@ diag: 64:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 68:6 error "use of undeclared identifier 'u'"
//@ diag: 69:1 error "expected ']'"
//@ diag: 68:4 note "to match this '['"
//@ diag: 71:15 error "expected ')'"
//@ diag: 71:12 note "to match this '('"
//@ diag: 71:13 warning "expression result unused"
//@ diag: 52:5 error "use of undeclared identifier 'u'"
//@ diag: 53:5 error "use of undeclared identifier 'u'"
//@ diag: 54:5 error "use of undeclared identifier 'u'"

int a[4];
void f(int, int);

void g() {
  f(u 8, "a"*2);
  f(1, u 8);
  a[u 8];
  a[u 8, 1];
  a[u 8] + "a"*2;
  (u 8) + "a"*2;
  (u) + "a"*2;
  ((u) 8);
  (u + 1 8);
  (1 + u 8);
  (u ? 1 : 2 8);
  (1 + ) + "a"*2;
  int x = (u 8, 1);
  int y = (u 8) + "a"*2;
  f((u) 8, "a"*2);
}

void h() {
  a[(u 8)];
}

void k() { (1 8); }
