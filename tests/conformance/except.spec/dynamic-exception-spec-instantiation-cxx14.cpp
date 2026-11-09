// C++14 [except.spec]/2,13 and [temp.inst]: dependent and pack-expanded
// dynamic exception specifications are substituted when the specification
// is needed (DR1330) and each substituted type is checked then; a failing
// type makes the specification ill-formed with an instantiation note.  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 41:35 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 46:35 error "pointer to incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 47:64 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 43:36 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 49:3 note "in instantiation of exception specification for 'f3<Inc>' requested here"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 43:36 error "rvalue reference type 'int &&' is not allowed in exception specification"
//@ diag: 50:3 note "in instantiation of exception specification for 'f3<int &&>' requested here"
//@ diag: 45:44 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 51:3 note "in instantiation of exception specification for 'f5<int, Inc>' requested here"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 44:37 error "pointer to incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 52:3 note "in instantiation of exception specification for 'f4<Inc>' requested here"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 47:46 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 53:15 note "in instantiation of exception specification for 'g' requested here"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 55:36 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 55:24 note "in instantiation of exception specification for 'f7<Inc>' requested here"
//@ diag: 56:15 note "in instantiation of function template specialization 'f7<Inc>' requested here"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 57:35 error "incomplete type 'void' is not allowed in exception specification"
//@ diag: 58:51 error "incomplete type 'typename Q::type' (aka 'Inc') is not allowed in exception specification"
//@ diag: 60:15 note "in instantiation of exception specification for 'f9<Q>' requested here"
//@ diag: 39:8 note "forward declaration of 'Inc'"
//@ diag: 43:36 error "incomplete type 'void' is not allowed in exception specification"
//@ diag: 60:24 note "in instantiation of exception specification for 'f3<void>' requested here"
struct Inc;
struct Abs { virtual void f() = 0; };
template<class T> void f1() throw(Inc);
template<class T> void f2() throw(Abs);
template<class T> void f3() throw(T);
template<class T> void f4() throw(T*);
template<class... Ts> void f5() throw(Ts...);
template<class T> void f6() throw(Inc*);
template<class T> struct S { void g() throw(T); void h() throw(Inc); };
void use() {
  f3<Inc>();
  f3<int&&>();
  f5<int, Inc>();
  f4<Inc>();
  S<Inc> s; s.g();
}
template<class T> void f7() throw(T) {}
template void f7<Inc>();
template<class T> void f8() throw(void);
template<class T> void f9() throw(typename T::type);
struct Q { typedef Inc type; };
void use2() { f9<Q>(); f3<void>(); f3<Abs>(); }
