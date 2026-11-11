// C23 5.1.2.3.2p1 (program startup): in ISO C modes clang rejects a main
// whose return type is not int; GNU modes only warn
// (tests/c/decl/main-return-type-void-gnu.c).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:1 error "'main' must return 'int'"
void main(void) {}
