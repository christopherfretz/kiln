//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "call to 'f' is ambiguous"
//@ diag: 10:21 note "candidate function [with T = int]"
//@ diag: 11:21 note "candidate function [with T = int]"
// [temp.func.order]/6: constraints that are not comparable leave the call
// ambiguous.
template <class T> concept A = sizeof(T) > 1;
template <class T> concept B = sizeof(T) > 2;
template <A T> void f(T);
template <B T> void f(T);
void g() { f(1); }
