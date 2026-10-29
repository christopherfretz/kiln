// [temp.inst]/3, [dcl.array]/1: a substituted array bound must be
// positive; the bound's value is checked at instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:39 error "array is too large (18446744073709551609 elements)"
//@ diag: 10:11 note "in instantiation of template class 'Arr<char>' requested here"
//@ diag: 11:40 error "'buf' declared as an array with a negative size"
//@ diag: 12:9 note "in instantiation of template class 'Buf<-1>' requested here"
template <class T> struct Arr { int a[sizeof(T) - 8]; };
Arr<char> a;
template <int N> struct Buf { char buf[N]; };
Buf<-1> b;
Buf<4> ok;
