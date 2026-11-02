// [temp.inst]/3, [dcl.array]/1: a substituted array bound must be
// positive; the bound's value is checked at instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:39 error "array is too large (18'446'744'073'709'551'609 elements)"
//@ diag: 12:11 note "in instantiation of template class 'Arr<char>' requested here"
//@ diag: 13:40 error "'buf' declared as an array with a negative size"
//@ diag: 14:9 note "in instantiation of template class 'Buf<-1>' requested here"
//@ requires: clang>=22
// clang 22 spelling of temp.inst-array-bound.cpp (WP-377).
template <class T> struct Arr { int a[sizeof(T) - 8]; };
Arr<char> a;
template <int N> struct Buf { char buf[N]; };
Buf<-1> b;
Buf<4> ok;
