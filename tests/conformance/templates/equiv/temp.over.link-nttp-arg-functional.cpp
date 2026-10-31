//@ expect: compile-fail
//@ diag: 10:9 error "call to 'f' is ambiguous"
//@ diag: 8:22 note "candidate function [with N = 2]"
//@ diag: 9:22 note "candidate function [with N = 2]"
// [temp.over.link]/6: `N + 1` and `1 + N` are functionally equivalent but
// not equivalent: two templates (ill-formed NDR; diagnosed as clang does).
template <int N> struct X {};
template <int N> int f(X<N + 1>);
template <int N> int f(X<1 + N>);
int r = f<2>(X<3>());
