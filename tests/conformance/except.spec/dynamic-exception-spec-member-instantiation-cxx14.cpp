// C++14 [except.spec]/2,13: dynamic exception specifications of class
// template members are instantiated on demand; an overrider's dependent
// list is checked against the base eagerly and per specialization.  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 33:39 error "exception specification of overriding function is more lax than base version"
//@ diag: 32:25 note "overridden virtual function is here"
//@ diag: 30:14 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 35:10 note "in instantiation of exception specification for 'S' requested here"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 27:19 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 35:15 note "in instantiation of exception specification for 'g' requested here"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 29:40 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 35:22 note "in instantiation of exception specification for 'h<int>' requested here"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 33:39 error "exception specification of overriding function is more lax than base version"
//@ diag: 37:21 note "in instantiation of template class 'D<char>' requested here"
//@ diag: 32:25 note "overridden virtual function is here"
//@ diag: 42:35 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 43:33 note "in instantiation of exception specification for 'r<Inc>' requested here"
//@ diag: 25:8 note "forward declaration of 'Inc'"
struct Inc;
template<class T> struct S {
  void g() throw(T);
  virtual void v() throw(T*);
  template<class U> void h() throw(T, U);
  S() throw(T);
};
struct B { virtual void k() throw(int); };
template<class T> struct D : B { void k() throw(T); };
void use() {
  S<Inc> s; s.g(); s.h<int>();
  S<Inc*>* p = 0; (void)p;
  D<int> d; D<char> e;
  void (*fp)() throw(int) = 0; (void)fp;
  bool b = noexcept(s.g());
}
template<class T> void q() throw(T) {}
template<class T> void r() throw(T) {}
void use2() { bool b = noexcept(r<Inc>()); (void)b; }
