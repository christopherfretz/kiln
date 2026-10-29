// [namespace.udecl]/11: a target that conflicts with a declaration already
// in the scope is ill-formed (a function of the same type, a variable, a
// class against a class)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:10 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 15:20 note "target of using declaration"
//@ diag: 16:6 note "conflicting declaration"
//@ diag: 19:10 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 15:32 note "target of using declaration"
//@ diag: 18:5 note "conflicting declaration"
//@ diag: 21:10 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 15:42 note "target of using declaration"
//@ diag: 20:8 note "conflicting declaration"
namespace M { void f(int); int v; struct S {}; }
void f(int);
using M::f;
int v;
using M::v;
struct S;
using M::S;
