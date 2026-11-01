// [temp.spec.partial.general]/8, [temp.deduct.type]/5: a member class template partial specialization
// whose template parameter is not deducible from its arguments gets -Wunusable-partial-specialization
// (an error by default), in-class and out-of-line; one using none of its parameters is rejected
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:63 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 18:43 note "non-deducible template parameter 'V'"
//@ diag: 20:55 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 20:34 note "non-deducible template parameter 'U'"
//@ diag: 21:88 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 21:78 note "non-deducible template parameter 'U'"
//@ diag: 23:55 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 23:34 note "non-deducible template parameter 'U'"
//@ diag: 24:88 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 24:78 note "non-deducible template parameter 'U'"
//@ diag: 25:69 error "partial specialization of 'In' does not use any of its template parameters"
template<class T> struct Outer { template<class U, class V> struct Inner; };
template<class T> template<class U, class V> struct Outer<T>::Inner<U*, int> {};
template<class T> struct Outer2 { template<class U> struct Inner; };
template<class T> template<class U> struct Outer2<T>::Inner<T*> {};
template<class T> struct O2 { template<class U> struct In {}; template<class U> struct In<T> {}; };
template<class T> struct Outer3 { template<class U> struct Inner; };
template<class T> template<class U> struct Outer3<T>::Inner<int> {};
template<class T> struct O3 { template<class U> struct In {}; template<class U> struct In<int> {}; };
struct S { template<class U> struct In {}; template<class U> struct In<int> {}; };
