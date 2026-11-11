// C23 6.8.5.3p3 (the switch statement): a case label's expression is an
// integer constant expression, converted to the promoted type of the
// controlling expression; a floating constant is not one.
// gcc.c-torture: compile/pr69989-2.c.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:8 error "integer constant expression must have integer type, not 'double'"
int f(int op) {
  switch (op) {
  case 0xca093fa0:
    return 1;
  case 1.0:
    return 3;
  case 300L:
    return 2;
  }
  return 0;
}
