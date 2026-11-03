// [expr.prim.id.unqual]/1 (recovery): under clang 22 an undeclared plain
// identifier is an error expression; parsing goes on when an operator,
// postfix or closing token follows it (otherwise the operand list is
// skipped). An invalid parenthesized operand skips silently to its ')'
// and the enclosing expression goes on: `(u 8) + "a"*2` still diagnoses
// the right operand. -19 is the clang 19 counterpart.
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 44:5 error "use of undeclared identifier 'u'"
//@ diag: 44:13 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 45:8 error "use of undeclared identifier 'u'"
//@ diag: 46:5 error "use of undeclared identifier 'u'"
//@ diag: 47:5 error "use of undeclared identifier 'u'"
//@ diag: 48:5 error "use of undeclared identifier 'u'"
//@ diag: 48:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 49:4 error "use of undeclared identifier 'u'"
//@ diag: 49:14 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 50:4 error "use of undeclared identifier 'u'"
//@ diag: 50:12 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 51:5 error "use of undeclared identifier 'u'"
//@ diag: 52:4 error "use of undeclared identifier 'u'"
//@ diag: 53:8 error "use of undeclared identifier 'u'"
//@ diag: 54:4 error "use of undeclared identifier 'u'"
//@ diag: 55:8 error "expected expression"
//@ diag: 55:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 56:12 error "use of undeclared identifier 'u'"
//@ diag: 57:12 error "use of undeclared identifier 'u'"
//@ diag: 57:22 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 58:6 error "use of undeclared identifier 'u'"
//@ diag: 58:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 62:6 error "use of undeclared identifier 'u'"
//@ diag: 62:11 error "expected ']'"
//@ diag: 62:4 note "to match this '['"
//@ diag: 65:15 error "expected ')'"
//@ diag: 65:12 note "to match this '('"
//@ diag: 65:13 warning "expression result unused"

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
