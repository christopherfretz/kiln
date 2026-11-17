// GNU `extern inline` (with `gnu_inline`, GCC's inline-only definition)
// followed by another definition of the same function: GCC 16.1 accepts
// it in C++ and emits the second; clang 22 rejects it, in a GNU dialect
// with its own wording (`CheckForFunctionRedefinition`). kiln follows
// clang: at namespace scope, in a namespace, for a function template and
// for two `gnu_inline` definitions. Found running GCC's C++ testsuite.
// WP-1550.
//@ args: -std=gnu++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:5 error "redefinition of a 'extern inline' function 'f' is not supported in C++"
//@ diag: 21:47 note "previous definition is here"
//@ diag: 24:8 error "redefinition of a 'extern inline' function 'g' is not supported in C++"
//@ diag: 23:61 note "previous definition is here"
//@ diag: 26:21 error "redefinition of a 'extern inline' function 't' is not supported in C++"
//@ diag: 25:63 note "previous definition is here"
//@ diag: 28:47 error "redefinition of a 'extern inline' function 'h' is not supported in C++"
//@ diag: 27:47 note "previous definition is here"
//@ diag: 30:5 error "redefinition of a 'extern inline' function 'k' is not supported in C++"
//@ diag: 29:19 note "previous definition is here"
extern inline __attribute__((gnu_inline)) int f(int x) { return 2; }
int f(int x) { return 1; }
namespace N { extern inline __attribute__((gnu_inline)) int g(int x) { return 2; } }
int N::g(int x) { return 1; }
template<class T> extern inline __attribute__((gnu_inline)) T t(T x) { return 2; }
template<class T> T t(T x) { return 1; }
extern inline __attribute__((gnu_inline)) int h(int x) { return 2; }
extern inline __attribute__((gnu_inline)) int h(int x) { return 3; }
extern inline int k(int x) { return 2; }
int k(int x) { return 1; }
