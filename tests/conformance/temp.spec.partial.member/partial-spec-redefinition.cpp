// [temp.spec.partial.general]/1, [basic.def.odr]/1: redefining a class template partial specialization
// (namespace scope, in-class member, out-of-line member) names it as written ('Inner<U *>')
// and notes the previous definition
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:54 error "redefinition of 'Inner<U *>'"
//@ diag: 18:54 note "previous definition is here"
//@ diag: 20:103 error "redefinition of 'X<T *>'"
//@ diag: 20:68 note "previous definition is here"
//@ diag: 21:118 error "redefinition of 'I<U *>'"
//@ diag: 21:83 note "previous definition is here"
//@ diag: 25:50 error "redefinition of 'I<U *>'"
//@ diag: 23:50 note "previous definition is here"
//@ diag: 29:26 error "redefinition of 'X<T *>'"
//@ diag: 28:26 note "previous definition is here"
template<class T> struct Outer { template<class U> struct Inner; };
template<class T> template<class U> struct Outer<T>::Inner<U*>;
template<class T> template<class U> struct Outer<T>::Inner<U*> {};
template<class T> template<class U> struct Outer<T>::Inner<U*> {};
namespace N { template<class T> struct X; template<class T> struct X<T*> {}; template<class T> struct X<T*> {}; }
template<class T> struct A { template<class U> struct I; template<class U> struct I<U*> {}; template<class U> struct I<U*> {}; };
template<class T> struct B { template<class U> struct I; };
template<class T> template<class U> struct B<T>::I<U*> {};
template<class T> template<class U> struct B<T>::I<U*>;
template<class T> template<class U> struct B<T>::I<U*> {};
template<class T> struct X;
template<class T> struct X<T*>;
template<class T> struct X<T*> {};
template<class T> struct X<T*> {};
