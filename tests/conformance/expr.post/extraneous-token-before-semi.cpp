// [expr.post]/1, [stmt.expr]/1: an extra ')' or ']' directly before the ';'
// of an expression statement or a simple-declaration is diagnosed as
// "extraneous ']' before ';'" (removal fix-it) and skipped, as clang's
// ExpectAndConsumeSemi does; a return statement says "expected ';' after
// return statement" instead. Same under clang 19 and 22.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 22:13 error "extraneous ']' before ';'"
//@ diag: 25:7 error "extraneous ']' before ';'"
//@ diag: 26:7 error "extraneous ')' before ';'"
//@ diag: 27:7 error "expected ';' after expression"
//@ diag: 27:7 error "expected expression"
//@ diag: 28:15 error "extraneous ']' before ';'"
//@ diag: 29:14 error "extraneous ')' before ';'"
//@ diag: 30:14 error "expected ';' after return statement"
//@ diag: 25:6 warning "expression result unused"
//@ diag: 27:6 warning "expression result unused"

int a[4];
void f(int);
int b = a[1]];

int g() {
  a[1]];
  f(1));
  a[1]] + 2;
  int x = a[1]];
  int y = (1));
  return a[2]];
}
