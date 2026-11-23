// [dcl.fct]/3, [dcl.attr.grammar]/5: a parameter of a function type
// inside a declarator (of a pointer to function, a function's return
// type, a typedef, a parameter's type) is a parameter declaration too,
// and its attributes are checked as clang checks every `ParmVarDecl`
// (clang `ActOnParamDeclarator` → `ProcessDeclAttributes`).
// spec/sema.md §4.5 "Parameter attributes", WP-2476.
//@ verify
void (*fp)(int y __attribute__((nonnull))); // expected-warning {{'nonnull' attribute only applies to pointer arguments}}
void (*f(int x))(int y __attribute__((nonnull)), int *__attribute__((align_value(3))) w); // expected-warning {{'nonnull' attribute only applies to pointer arguments}} expected-error {{requested alignment is not a power of 2}}
typedef void T(int y __attribute__((noescape))); // expected-warning {{'noescape' attribute only applies to pointer arguments}}
void g(void (*cb)(int q __attribute__((nonnull)))); // expected-warning {{'nonnull' attribute only applies to pointer arguments}}
using U = void (*)(int *__attribute__((nonnull, noescape)) p, int *__attribute__((align_value(16))) q);
// Checked once, at the template, not again per instantiation.
template <class T> struct S { void m(void (*cb)(int q __attribute__((nonnull)))) {} }; // expected-warning {{'nonnull' attribute only applies to pointer arguments}}
template <class T> void f2(void (*cb)(int q __attribute__((nonnull)))) {} // expected-warning {{'nonnull' attribute only applies to pointer arguments}}
void u() { S<int>().m(0); S<long>().m(0); f2<int>(0); }
