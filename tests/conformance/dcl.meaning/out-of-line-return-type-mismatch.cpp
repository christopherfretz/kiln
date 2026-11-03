// [dcl.meaning.general]/2, [class.mfct]/2: an out-of-line member
// function definition that differs from the declared member only in its
// return type is reported as such, with the declaration noted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:9 error "return type of out-of-line definition of 'C::g' differs from that in the declaration"
//@ diag: 12:16 note "previous declaration is here"
//@ diag: 15:31 error "return type of out-of-line definition of 'D::g' differs from that in the declaration"
//@ diag: 14:35 note "previous declaration is here"
//@ diag: 18:36 error "return type of out-of-line definition of 'P<type-parameter-0-0, int>::g' differs from that in the declaration"
//@ diag: 17:43 note "previous declaration is here"
struct C { int g(int); };
long C::g(int) { return 0; }
template <class T> struct D { int g(int); };
template <class T> long D<T>::g(int) { return 0; }
template <class T, class U> struct P { int g(int); };
template <class T> struct P<T, int> { int g(int); };
template <class T> long P<T, int>::g(int) { return 0; }
