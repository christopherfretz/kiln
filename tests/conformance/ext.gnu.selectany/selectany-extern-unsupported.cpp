// GNU selectany on an 'extern' declaration without an initializer
// (WP-809): clang makes `__attribute__((selectany)) extern int f;` a weak_odr
// definition (zero-initialized). kiln rejects it rather than guess.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang makes it a definition; kiln rejects it (WP-809)
//@ diag: 9:16 error "'selectany' on an 'extern' declaration without an initializer is not supported yet"

__attribute__((selectany)) extern int f;
int g() { return f; }
