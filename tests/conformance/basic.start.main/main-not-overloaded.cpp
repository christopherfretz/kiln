// [basic.start.main]/3: `main` shall not be overloaded; a declaration with
// another signature conflicts with the first (clang treats `main` as never
// overloaded, then reports conflicting types; WP-1015).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "conflicting types for 'main'"
//@ diag: 11:5 note "previous declaration is here"
//@ diag: 13:5 error "conflicting types for 'main'"
//@ diag: 11:5 note "previous declaration is here"
int main(int, char **);
int main() { return 0; }
int main(int, char **, char **);
