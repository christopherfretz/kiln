// GNU __builtin_types_compatible_p takes exactly two type names.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:9 error "type trait requires 2 arguments; have 1 argument"
//@ diag: 8:9 error "type trait requires 2 arguments; have 3 arguments"
//@ diag: 9:43 error "expected a type"
int a = __builtin_types_compatible_p(int);
int b = __builtin_types_compatible_p(int, int, int);
int c = __builtin_types_compatible_p(int, 1);
