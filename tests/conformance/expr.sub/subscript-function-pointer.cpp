// [expr.sub]/2: the pointer operand of a subscript points to a completely
// defined object type, so a pointer to function is an error at that
// operand, either operand order (clang's err_subscript_function_type).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:9 error "subscript of pointer to function type 'void ()'"
//@ diag: 15:11 error "subscript of pointer to function type 'void ()'"
//@ diag: 16:9 error "subscript of pointer to function type 'fn' (aka 'void (int)')"
//@ diag: 17:11 error "subscript of pointer to function type 'void ()'"
void f();
using fn = void (int);

void subscripts(void (*fp)(), fn *q, int i) {
  (void)fp[0];
  (void)0[fp];
  (void)q[i];
  int n = fp[1] + 1;
  (void)n;
}
