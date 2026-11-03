// [dcl.link]/6: a C-linkage declaration in another namespace redeclares
// the entity (clang's `checkForConflictWithNonVisibleExternC`): a
// different return type, parameter list or variable type, a second
// definition, or another kind of entity is an error. [dcl.link]/7: a
// global variable and a C-linkage entity of the same name in a namespace
// conflict; a global function does not (it overloads or redeclares).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:31 error "conflicting types for 'f'"
//@ diag: 27:30 note "previous declaration is here"
//@ diag: 30:30 error "conflicting types for 'f'"
//@ diag: 27:30 note "previous declaration is here"
//@ diag: 31:33 error "redeclaration of 'v' with a different type: 'double' vs 'int'"
//@ diag: 28:53 note "previous declaration is here"
//@ diag: 33:30 error "redefinition of 'f'"
//@ diag: 32:16 note "previous definition is here"
//@ diag: 35:16 error "conflicting types for 'g'"
//@ diag: 34:16 note "previous declaration is here"
//@ diag: 37:5 error "declaration of 'w' in global scope conflicts with declaration with C language linkage"
//@ diag: 36:30 note "declared with C language linkage here"
//@ diag: 39:30 error "declaration of 'u' with C language linkage conflicts with declaration in global scope"
//@ diag: 38:5 note "declared in global scope here"
//@ diag: 41:30 error "redefinition of 'k' as different kind of symbol"
//@ diag: 40:30 note "previous definition is here"
//@ requires: clang>=22
// clang 22 spelling of across-namespaces-errors.cpp (WP-394).
namespace A { extern "C" int f(int); }
namespace B { extern "C" int f(int); extern "C" int v; }
namespace C { extern "C" long f(int); }
namespace D { extern "C" int f(double); }
namespace E { extern "C" double v; }
extern "C" int f(int) { return 0; }
namespace F { extern "C" int f(int) { return 1; } }
extern "C" int g(int);
extern "C" int g(long);
namespace G { extern "C" int w; }
int w;
int u;
namespace H { extern "C" int u(); }
namespace I { extern "C" int k(); }
namespace J { extern "C" int k; }
namespace K { extern "C" int h(); }
int h();
int h(int);
