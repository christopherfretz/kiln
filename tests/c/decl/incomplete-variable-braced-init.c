// C17 6.7.9p3 / clang `CheckCompleteVariableDeclaration`: a variable of
// incomplete type with a braced initializer is reported at the variable's
// name, not at the `{` (WP-1312, WP-1305 follow-up).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:10 error "variable has incomplete type 'struct S'"
//@ diag: 10:8 note "forward declaration of 'struct S'"
//@ diag: 12:25 error "variable has incomplete type 'struct S'"
//@ diag: 10:8 note "forward declaration of 'struct S'"
struct S;
struct S s = { 1 };
void f(void) { struct S t = { 1 }; (void)&t; }
