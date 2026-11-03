// [stmt.pre]/5, [dcl.type.auto.deduct]/5: a condition declaration's
// decltype(auto) placeholder naming a function deduces the function type: the
// initialization fails, and clang's CheckConditionVariable then says "a
// function type is not allowed here" at the declarator.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:31 error "cannot initialize a variable of type 'int (int)' with an lvalue of type 'int (int)'"
//@ diag: 16:31 error "a function type is not allowed here"
//@ diag: 17:35 error "cannot initialize a variable of type 'int (int)' with an lvalue of type 'int (int)'"
//@ diag: 17:35 error "a function type is not allowed here"
//@ diag: 18:28 error "cannot initialize a variable of type 'int (int)' with an lvalue of type 'int (int)'"
int fn(int);
// decltype(auto) deduces the function type, which no initializer converts
// to; a condition variable of function type is also rejected.

void z() { if (decltype(auto) x = fn) {} }
void z2() { while (decltype(auto) x = fn) {} }
void z3() { decltype(auto) y = fn; }
