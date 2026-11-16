// Too many arguments to a function-like macro because of the commas of a
// braced initializer list ([cpp.replace.general]/12: only parentheses
// group arguments): as clang 22, the error adds the note that
// parentheses are required, and the invocation then proceeds with each
// such argument parenthesized, so nothing else is diagnosed. An argument
// that starts with the braced list cannot be fixed that way: the note
// says so and the invocation is dropped (so `M` is then an undeclared
// identifier). Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:14 error "too many arguments provided to function-like macro invocation"
//@ diag: 22:9 note "macro 'M' defined here"
//@ diag: 24:7 note "parentheses are required around macro argument containing braced initializer list"
//@ diag: 25:26 error "too many arguments provided to function-like macro invocation"
//@ diag: 22:9 note "macro 'M' defined here"
//@ diag: 25:10 note "parentheses are required around macro argument containing braced initializer list"
//@ diag: 26:18 error "too many arguments provided to function-like macro invocation"
//@ diag: 22:9 note "macro 'M' defined here"
//@ diag: 26:12 note "cannot use initializer list at the beginning of a macro argument"
//@ diag: 26:12 error "use of undeclared identifier 'M'"
#define M(x) x
struct S { int a, b; };
S s = M(S{1, 2});
int *p = M(new int[2]{1, 2});
int q[2] = M({1, 2});
