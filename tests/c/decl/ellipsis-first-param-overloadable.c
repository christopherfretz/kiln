// C17 6.7.6p1: `(...)` alone is ill-formed before C23, but clang accepts
// it on an `overloadable` function in every C dialect (its <tgmath.h>
// declares `__tg_promote(...)` so), the attribute in the decl-specifiers
// or after the declarator; it covers only the declarator it belongs to.
// WP-1331.
//@ args: -std=gnu17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:8 error "ISO C requires a named parameter before '...'"
//@ diag: 16:11 error "ISO C requires a named parameter before '...'"
//@ diag: 17:46 error "ISO C requires a named parameter before '...'"
void f(...) __attribute__((overloadable));
__attribute__((overloadable)) void g(...);
void __attribute__((overloadable)) h(...);
void k(...);
void (*p)(...);
void m(...) __attribute__((overloadable)), n(...);
