// [cpp.pragma]: a '#pragma weak' without an identifier, or with tokens
// after it, is ignored with a warning (clang PragmaWeakHandler).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 7:13 warning "expected identifier in '#pragma weak' - ignored"
//@ diag: 8:16 warning "extra tokens at end of '#pragma weak' - ignored"
#pragma weak
#pragma weak f g
extern "C" void f();
