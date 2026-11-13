// [cpp.pragma]: two '#pragma clang attribute' diagnostics in clang 19's
// form, which kiln follows: 'visibility' is not supported by the pragma
// (clang 22 supports it), and the invalid matchers read "can't" (clang 22:
// "cannot"); the attribute is still pushed. WP-1399.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "attribute 'visibility' is not supported by '#pragma clang attribute'"
//@ diag: 11:15 error "attribute 'cold' can't be applied to 'enum', and 'variable'"
#pragma clang attribute push (__attribute__((visibility("hidden"))), apply_to = function)
#pragma clang attribute push (__attribute__((cold)), apply_to = any(function, variable, enum))
void f();
#pragma clang attribute pop
