// [expr.throw]/2: the exception object's type must be complete (or a
// pointer to cv void); a throw-expression is an assignment-expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:3 error "cannot throw object of incomplete type 'I'"
//@ diag: 11:8 note "forward declaration of 'I'"
//@ diag: 14:3 error "cannot throw pointer to object of incomplete type 'const I'"
//@ diag: 11:8 note "forward declaration of 'I'"
//@ diag: 16:15 error "expected expression"
//@ diag: 17:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'void'"
struct I;
void f(I &ri, const I *pci, void *pv, int x) {
  throw ri;
  throw pci;
  throw pv;
  int a = 1 + throw 2;
  int b = (throw x);
}
