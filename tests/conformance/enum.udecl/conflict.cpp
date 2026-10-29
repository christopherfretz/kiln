// [enum.udecl]/2, [namespace.udecl]/11: the enumerators are introduced as
// by using-declarations, so a later declaration of the name conflicts
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:5 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 11:30 note "target of using declaration"
//@ diag: 13:15 note "using declaration"
//@ diag: 17:7 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 11:33 note "target of using declaration"
//@ diag: 16:17 note "using declaration"
namespace N { enum class E { A, B }; }
using enum N::E;
using enum N::E;
int A;
void g() {
  using enum N::E;
  int B;
}
