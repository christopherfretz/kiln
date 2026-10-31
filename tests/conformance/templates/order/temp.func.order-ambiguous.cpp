// [temp.func.order]/6: neither template is more specialized; the call is ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:9 error "call to 'k' is ambiguous"
//@ diag: 7:24 note "candidate function [with T = int]"
//@ diag: 8:24 note "candidate function [with T = int]"
template <class T> int k(T, int);
template <class T> int k(int, T);
int b = k(1, 1);
