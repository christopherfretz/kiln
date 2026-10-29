// [namespace.udecl]/11: a declaration that conflicts with the target of a
// using-declaration already in the scope (namespace and block scope)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:6 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 14:20 note "target of using declaration"
//@ diag: 15:10 note "using declaration"
//@ diag: 18:5 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 14:32 note "target of using declaration"
//@ diag: 17:10 note "using declaration"
//@ diag: 21:7 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 14:32 note "target of using declaration"
//@ diag: 20:12 note "using declaration"
namespace P { void g(int); int v; }
using P::g;
void g(int);
using P::v;
int v;
void h() {
  using P::v;
  int v;
}
