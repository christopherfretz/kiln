// C99 inline semantics in a GNU dialect: an `extern inline` definition is
// an external definition, so another definition is an error, with clang's
// GNU-dialect wording (`CheckForFunctionRedefinition`); with `gnu_inline`
// the first is inline-only and the redefinition is valid
// (gnu89-extern-inline-redefinition.c). WP-1550.
//@ args: -std=gnu99
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:5 error "redefinition of a 'extern inline' function 'f' is not supported in C99 mode"
//@ diag: 12:19 note "previous definition is here"
extern inline __attribute__((gnu_inline)) int g(int x) { return 2; }
extern inline int f(int x) { return 2; }
int f(int x) { return 1; }
int g(int x) { return 1; }
