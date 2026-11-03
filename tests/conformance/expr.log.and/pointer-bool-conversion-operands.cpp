// [expr.log.and]/1, [expr.log.or]/1, [conv.bool]: each operand naming a
// function is converted to bool and warns (-Wpointer-bool-conversion),
// right operand first as clang's work list visits them; '&f' does not.
//@ strict-diags
//@ diag: 21:15 warning "address of function 'g' will always evaluate to 'true'"
//@ diag: 21:15 note "prefix with the address-of operator to silence this warning"
//@ diag: 21:10 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 21:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 22:10 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 22:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 23:19 warning "address of function 'g' will always evaluate to 'true'"
//@ diag: 23:19 note "prefix with the address-of operator to silence this warning"
//@ diag: 23:10 warning "address of function 'pred' will always evaluate to 'true'"
//@ diag: 23:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 23:10 note "suffix with parentheses to turn this into a function call"
void f();
void g();
bool pred();

void operands() {
  (void)(f && g);
  (void)(f || 1);
  (void)(pred && !g);
  (void)(&f && &g);
}
