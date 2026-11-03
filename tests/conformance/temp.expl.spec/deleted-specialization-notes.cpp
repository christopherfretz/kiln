// [temp.expl.spec]/1, [dcl.fct.def.delete]/2: a use of a deleted explicit
// specialization (of a function template, or of a member of a class template
// specialization) points at the specialization's declaration. As in clang, an
// overload candidate note says "implicitly deleted" there (the deletion is
// not on the declaration deduction formed), and a direct use says "'p<int>'
// has been explicitly marked deleted here". A member specialization
// `template<> void C<int>::q() = delete;` is not a deleted redeclaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:13 error "call to deleted function 'f'"
//@ diag: 33:17 note "candidate function [with T = int] has been implicitly deleted"
//@ diag: 36:13 error "call to deleted function 'g'"
//@ diag: 35:24 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 39:13 error "call to deleted function 'h'"
//@ diag: 38:17 note "candidate function [with T = long] has been implicitly deleted"
//@ diag: 39:20 error "call to deleted function 'h'"
//@ diag: 38:17 note "candidate function [with T = long] has been implicitly deleted"
//@ diag: 42:18 error "call to deleted member function 'm'"
//@ diag: 41:20 note "candidate function [with T = int] has been implicitly deleted"
//@ diag: 45:19 error "attempt to use a deleted function"
//@ diag: 44:17 note "'p<int>' has been explicitly marked deleted here"
//@ diag: 49:23 error "call to deleted member function 'q'"
//@ diag: 47:25 note "candidate function has been implicitly deleted"
//@ diag: 46:47 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 49:36 error "attempt to use a deleted function"
//@ diag: 48:24 note "'s' has been explicitly marked deleted here"
//@ diag: 50:21 error "attempt to use a deleted function"
//@ diag: 48:24 note "'s' has been explicitly marked deleted here"
//@ diag: 54:13 error "call to deleted function 'r'"
//@ diag: 52:17 note "candidate function [with T = char] has been implicitly deleted"
//@ diag: 53:6 note "candidate function"
template<class T> void f(T) {}
template<> void f<int>(int) = delete;
void k1() { f(1); }
template<class T> void g(T) = delete;
void k2() { g(1); }
template<class T> void h(T) {}
template<> void h(long) = delete;
void k3() { h(1L); h<long>(2); }
struct S { template<class T> void m(T) {} };
template<> void S::m<int>(int) = delete;
void k4(S s) { s.m(1); }
template<class T> void p(T) {}
template<> void p<int>(int) = delete;
void (*pp)(int) = &p<int>;
template<class T> struct C { void q() {} void q(int) {} static int s() { return 0; } };
template<> void C<int>::q() = delete;
template<> int C<int>::s() = delete;
void k5(C<int> c) { c.q(); C<int>::s(); }
auto *ps = &C<int>::s;
template<class T> void r(T, int) {}
template<> void r<char>(char, int) = delete;
void r(char, long);
void k6() { r('a', 1); }
