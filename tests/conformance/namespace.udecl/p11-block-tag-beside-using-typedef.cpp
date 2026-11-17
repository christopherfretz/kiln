// [namespace.udecl]/11, [basic.scope.scope]/4: in a block scope a class or
// enumeration and a typedef-name for another type correspond and declare
// different entities, so a using-declarator that brings in one beside the
// other declared first makes the program ill-formed (the order does not
// matter: either is reachable from the other). GCC 16.1 rejects these; clang
// 22 accepts them, as it does at namespace scope, where GCC accepts the
// typedef-name after the class too and kiln follows both
// (p11-class-beside-using-typedef.cpp, docs/status/deviations.md). Here the
// text and GCC decide. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 19 and 22 accept a block-scope class beside a using-declared typedef-name
//@ diag: 35:27 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 30:10 note "target of using declaration"
//@ diag: 35:15 note "conflicting declaration"
//@ diag: 36:27 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 31:8 note "target of using declaration"
//@ diag: 36:15 note "conflicting declaration"
//@ diag: 37:25 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 32:15 note "target of using declaration"
//@ diag: 37:10 note "conflicting declaration"
//@ diag: 40:22 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 32:15 note "target of using declaration"
//@ diag: 40:10 note "conflicting declaration"
//@ diag: 43:25 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 32:15 note "target of using declaration"
//@ diag: 43:10 note "conflicting declaration"
namespace N {
  struct S {};
  enum E { e };
  typedef int T;
}
void f() {
  typedef int S; using N::S;
  typedef int E; using N::E;
  struct T {}; using N::T;
}
void g() {
  struct T; using N::T;
}
void h() {
  struct T *p; using N::T;
}
