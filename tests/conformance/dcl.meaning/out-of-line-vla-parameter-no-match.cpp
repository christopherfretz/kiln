// [dcl.meaning.general]/2, [basic.scope.scope]/4, [dcl.array] (the
// variable length array extension in C++): a parameter of a variably
// modified type has a type of its own, the same as no other one
// (tests/conformance/dcl.array/vla-types-distinct.cpp), so an out-of-line
// definition or declaration of a member, or a qualified one of a
// namespace member, whose parameter is such a type matches no
// declaration, even with the same bound written (as clang 19 and 22;
// kiln does not emit clang's notes naming the parameter). WP-1405.
//@ expect: compile-fail
//@ args: -Wno-vla-cxx-extension
//@ diag: 25:9 error "out-of-line definition of 'f' does not match any declaration in 'S'"
//@ diag: 26:9 error "out-of-line definition of 'g' does not match any declaration in 'S'"
//@ diag: 27:9 error "out-of-line definition of 'h' does not match any declaration in 'S'"
//@ diag: 28:9 error "out-of-line definition of 'k' does not match any declaration in 'S'"
//@ diag: 29:9 error "out-of-line declaration of 'd' does not match any declaration in 'S'"
//@ diag: 32:9 error "out-of-line definition of 'q' does not match any declaration in namespace 'N'"
struct S {
  void f(int n, int (*p)[n]);
  void g(int n, int (*p)[n]);
  void h(int n, int (&p)[n]);
  void k(int n, int (*p)[2][n]);
  void d(int n, int (*p)[n]);
  void e(int n, int p[n]);
};
void S::f(int n, int (*p)[n]) {}
void S::g(int m, int (*p)[m + 1]) {}
void S::h(int n, int (&p)[n]) {}
void S::k(int n, int (*p)[2][n]) {}
void S::d(int n, int (*p)[n]);
void S::e(int n, int p[n]) {}
namespace N { void q(int n, int (*p)[n]); }
void N::q(int n, int (*p)[n]) {}
