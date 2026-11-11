// C89 6.3.2.2 (function calls), C23 6.2.1p4: an implicit function
// declaration has block scope. Each block that calls an undeclared
// function declares it again; a later block-scope declaration that is
// out of scope at the call is still found, with clang's
// -Wout-of-scope-function, and a later incompatible declaration conflicts
// with the implicit one. C99 and later reject the call itself
// (an error by default in clang).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:18 error "call to undeclared function 'foo'; ISO C99 and later do not support implicit function declarations"
//@ diag: 22:28 error "call to undeclared function 'foo'; ISO C99 and later do not support implicit function declarations"
//@ diag: 23:6 error "conflicting types for 'foo'"
//@ diag: 22:18 note "previous implicit declaration is here"
//@ diag: 24:18 error "call to undeclared function 'bar'; ISO C99 and later do not support implicit function declarations"
//@ diag: 24:28 error "call to undeclared function 'bar'; ISO C99 and later do not support implicit function declarations"
//@ diag: 25:16 error "call to undeclared function 'bar'; ISO C99 and later do not support implicit function declarations"
//@ diag: 27:16 warning "use of out-of-scope declaration of 'qux'"
//@ diag: 26:21 note "previous declaration is here"
//@ diag: 28:16 error "call to undeclared function 'baz'; ISO C99 and later do not support implicit function declarations"
//@ diag: 29:21 error "conflicting types for 'baz'"
//@ diag: 28:16 note "previous implicit declaration is here"
void f(void) { { foo(1); } foo(2); }
long foo(int);
void g(void) { { bar(1); } bar(2); }
void h(void) { bar(3); }
void a(void) { long qux(int); }
void b(void) { qux(1); }
void e(void) { baz(1); }
void k(void) { long baz(int); }
