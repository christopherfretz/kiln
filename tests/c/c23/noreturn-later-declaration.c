// C23 6.7.12.7 (and 6.7.4 `_Noreturn`): C has no counterpart of C++'s
// [dcl.attr.noreturn]/1 rule that the first declaration must carry the
// attribute; clang checks it in C++ only (`MergeFunctionDecl`), so a
// later declaration may add `[[noreturn]]` or `_Noreturn` (WP-1311; the
// C++ rule: tests/conformance/dcl.attr.noreturn/first-declaration.cpp).
//@ std: c23
//@ expect: compile-ok
//@ strict-diags
void f(void);
[[noreturn]] void f(void);
void g(void);
_Noreturn void g(void);
void h(void);
__attribute__((noreturn)) void h(void);
