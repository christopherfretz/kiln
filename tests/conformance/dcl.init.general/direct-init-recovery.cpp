// [dcl.init.general]/1, [expr.assign]/1: a malformed direct-initializer's
// expression-list is skipped through its `)` and the declaration goes on
// (a missing `;` is reported, then the rest of a braced body is skipped:
// clang `ParseDeclarationAfterDeclaratorAndAttributes`, `SkipMalformedDecl`).
// An operand missing before a binary operator is diagnosed, and the
// operator's right operand is parsed too (clang `ParseAssignmentExpression`).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:6 error "variable has incomplete type 'void'"
//@ diag: 21:9 error "expected expression"
//@ diag: 21:10 error "expected expression"
//@ diag: 21:11 error "expected ';' after top level declarator"
//@ diag: 22:11 error "expected ';' after top level declarator"
//@ diag: 25:9 error "expected expression"
//@ diag: 25:10 error "expected expression"
//@ diag: 26:9 error "expected expression"
//@ diag: 27:12 error "expected expression"
//@ diag: 28:11 error "expected ')'"
//@ diag: 28:8 note "to match this '('"
void f4(=) {}
int f5(+3) = 0;
int z = 1;
void g() {
  int a(=);
  int b(*= 2), c = 3;
  int d = (|| 1);
  int e(1 2);
}
int w = 2;
