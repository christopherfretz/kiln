// [dcl.fct.def.delete]: "call to deleted function" names the function as
// looked up ('show'), not the selected specialization ('show<int>'), as
// clang does (ULE->getName()); qualified calls name the unqualified name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:12 error "call to deleted function 'show'"
//@ diag: 22:24 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 25:21 error "call to deleted function 'show'"
//@ diag: 22:24 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 25:35 error "call to deleted function 'many'"
//@ diag: 23:27 note "candidate function [with T = <int, int>] has been explicitly deleted"
//@ diag: 25:51 error "call to deleted member function 'm'"
//@ diag: 24:35 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 27:16 error "attempt to use a deleted function"
//@ diag: 26:21 note "'q' has been explicitly marked deleted here"
//@ diag: 27:22 error "call to deleted function 'r'"
//@ diag: 26:61 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 27:32 error "call to deleted function 'r'"
//@ diag: 26:61 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 27:47 error "call to deleted function 'adl'"
//@ diag: 26:94 note "candidate function has been explicitly deleted"
template<class T> void show(T) = delete;
template<class... T> void many(T...) = delete;
struct S { template<class T> void m(T) = delete; };
void f() { show(1); show<int>(2); many(1, 2); S{}.m(1); }
namespace ns { void q(int) = delete; template<class T> void r(T) = delete; struct A {}; void adl(A) = delete; }
void g() { ns::q(1); ns::r(1); ns::r<int>(1); adl(ns::A{}); }
