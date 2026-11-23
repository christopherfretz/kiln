// [dcl.fct]/5, [basic.link]/11: in C++ a parameter's `noescape` is part
// of the function type (clang's `ExtParameterInfo`), so two declarations
// whose parameters differ in it declare the same function with different
// types: "conflicting types" (clang `MergeFunctionDecl`), for an
// out-of-line member, a function template, and a block-scope prior
// declaration alike. Declarations that agree are fine. spec/sema.md §4.5
// "Parameter attributes", WP-2476.
//@ verify
struct S { void m(int *__attribute__((noescape)) p); }; // expected-note {{previous declaration is here}}
void S::m(int *p) {} // expected-error {{conflicting types for 'm'}}
template <class T> void t(T *__attribute__((noescape)) p); // expected-note {{previous declaration is here}}
template <class T> void t(T *p) {} // expected-error {{conflicting types for 't'}}
void g() { void h(int *__attribute__((noescape)) p); } // expected-note {{previous declaration is here}}
void h(int *p); // expected-error {{conflicting types for 'h'}}
void k(int *__attribute__((noescape)) p);
void k(int *__attribute__((noescape)) p) {}
void n(int *p);
void n(int *p) {}
