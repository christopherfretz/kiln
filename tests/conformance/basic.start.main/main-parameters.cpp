// [basic.start.main]/2: `main` takes no parameters, `(int, char**)`, or
// clang's implementation-defined `(int, char**, char**)` (clang's
// err_main_surplus_args / err_main_arg_wrong, at the name). A `main` so
// declared is invalid and conflicts with nothing (WP-1015).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:5 error "too many parameters (4) for 'main': must be 0, 2, or 3"
//@ diag: 13:5 error "first parameter of 'main' (argument count) must be of type 'int'"
//@ diag: 13:5 error "third parameter of 'main' (environment) must be of type 'char **'"
//@ diag: 14:5 error "second parameter of 'main' (argument array) must be of type 'char **'"
//@ diag: 15:5 error "second parameter of 'main' (argument array) must be of type 'char **'"
int main(float a, char **argv, int envp, int x);
int main(int, int);
int main(int, volatile char **);
