// C++14 [except.spec]/14-15: implicit destructor exception specifications
// computed from member type lists, checked against out-of-line definitions
// of class-template members at instantiation.  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 27:26 error "exception specification in declaration does not match previous declaration"
//@ diag: 26:36 note "previous declaration is here"
//@ diag: 31:26 error "exception specification in declaration does not match previous declaration"
//@ diag: 30:31 note "previous declaration is here"
//@ diag: 33:26 error "exception specification in declaration does not match previous declaration"
//@ diag: 32:37 note "previous declaration is here"
//@ diag: 35:26 error "'~T4<T>' is missing exception specification 'throw(E1)'"
//@ diag: 34:31 note "previous declaration is here"
//@ diag: 38:26 error "'~T7<T>' is missing exception specification 'throw(E1)'"
//@ diag: 37:31 note "previous declaration is here"
//@ diag: 40:26 error "'~T5<T>' is missing exception specification 'noexcept'"
//@ diag: 39:31 note "previous declaration is here"
//@ diag: 42:26 error "'T6<T>' is missing exception specification 'throw(E1)'"
//@ diag: 41:31 note "previous declaration is here"
//@ diag: 43:31 error "'f' is missing exception specification 'throw(E1)'"
//@ diag: 41:52 note "previous declaration is here"
struct E1 {};
struct M1 { ~M1() throw(E1); };
template<class T> struct TT { T t; ~TT(); };
template<class T> TT<T>::~TT() throw() {}
TT<int> a;
TT<M1> b;
template<class T> struct T2 { ~T2(); };
template<class T> T2<T>::~T2() throw(E1) {}
template<class T> struct T3 { M1 m; ~T3(); };
template<class T> T3<T>::~T3() throw(E1) {}
template<class T> struct T4 { ~T4() throw(E1); };
template<class T> T4<T>::~T4() {}
T4<int> c;
template<class T> struct T7 { ~T7() throw(E1); };
template<class T> T7<T>::~T7() {}
template<class T> struct T5 { ~T5() noexcept; };
template<class T> T5<T>::~T5() {}
template<class T> struct T6 { T6() throw(E1); void f() throw(E1); };
template<class T> T6<T>::T6() {}
template<class T> void T6<T>::f() {}
