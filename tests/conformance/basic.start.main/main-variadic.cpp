// clang's -Wmain: `main` declared variadic is an extension warning at the
// name; a variadic `main` with a wrong parameter is only an error (WP-1040).
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ diag: 7:5 warning "'main' is not allowed to be declared variadic"
int main(int argc, char **argv, ...) { return argc + (argv == nullptr); }
