// C17 6.7p4, 6.2.7p2: all declarations of a function in a translation
// unit must have compatible types; C has no overloading.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:5 error "conflicting types for 'f'"
//@ diag: 9:5 note "previous declaration is here"
//@ diag: 12:5 error "conflicting types for 'g'"
//@ diag: 11:6 note "previous declaration is here"
int f(int);
int f(double);
void g(void);
int g(void);
