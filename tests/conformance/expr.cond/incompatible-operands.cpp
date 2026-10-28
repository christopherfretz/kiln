// [expr.cond]/7: the operands need a common type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:14 error "incompatible operand types ('int *' and 'double')"
int *p;
double d;
bool b;
void f() { b ? p : d; }
