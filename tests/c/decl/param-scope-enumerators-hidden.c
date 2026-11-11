// C23 6.2.1p4: an enumerator of an enumeration defined in a function
// declarator's parameter list is out of scope after the declarator, as
// its tag is (clang: "use of undeclared identifier").
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-visibility
//@ diag: 10:22 error "use of undeclared identifier 'X'"
//@ diag: 11:43 error "use of undeclared identifier 'K'"
int f(enum E { X = 5, Y } e);
int g(void) { return X; }
int h(void) { int k(enum { K } a); return K; }
