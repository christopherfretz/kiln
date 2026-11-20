// __builtin_invoke's argument count errors are clang's at the call: at
// least one argument, at least two for a pointer to member, exactly two
// for a pointer to data member; anything else is the error of the call
// it denotes (clang 22 BuiltinInvoke; WP-1740).
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 13:9 error "too few arguments to function call, expected at least 1, have 0"
//@ diag: 14:9 error "too few arguments to function call, expected at least 2, have 1"
//@ diag: 15:38 error "too many arguments to function call, expected 2, have 3"
//@ diag: 16:9 error "called object type 'int' is not a function or function pointer"
struct S { int v; int f(int); };
int a = __builtin_invoke();
int b = __builtin_invoke(&S::f);
int c = __builtin_invoke(&S::v, S{}, 1);
int d = __builtin_invoke(1, 2);
