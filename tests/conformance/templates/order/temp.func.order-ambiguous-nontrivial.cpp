// [temp.func.order]: f(T*, U) and f(T, U*) are unordered for (int*, int*).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "call to 'f' is ambiguous"
//@ diag: 7:33 note "candidate function [with T = int, U = int *]"
//@ diag: 8:33 note "candidate function [with T = int *, U = int]"
template <class T, class U> int f(T*, U);
template <class T, class U> int f(T, U*);
int* p;
int r = f(p, p);
