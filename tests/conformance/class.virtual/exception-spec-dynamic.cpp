// C++14 [except.spec]/5 (C++11/14): an overrider's dynamic exception specification
// must allow no more than the base's: each listed type must be caught by a handler
// of one of the base's types ([except.handle]/3: the same type, an unambiguous
// public base, a pointer qualification or derived-to-base conversion, void *, or
// std::nullptr_t; a reference handler to a pointer must be const). Found by the
// clang negative-test run (WP-912), fixed by WP-1017.
//@ std: c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 73:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 50:16 note "overridden virtual function is here"
//@ diag: 72:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 49:16 note "overridden virtual function is here"
//@ diag: 74:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 51:16 note "overridden virtual function is here"
//@ diag: 75:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 52:16 note "overridden virtual function is here"
//@ diag: 76:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 53:16 note "overridden virtual function is here"
//@ diag: 77:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 54:16 note "overridden virtual function is here"
//@ diag: 78:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 55:16 note "overridden virtual function is here"
//@ diag: 79:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 56:16 note "overridden virtual function is here"
//@ diag: 80:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 57:16 note "overridden virtual function is here"
typedef decltype(nullptr) nullptr_t;
struct A {};
struct B1 : A {};
struct B2 : A {};
struct D : B1, B2 {};
struct P : private A {};
struct C {};
struct E : C {};

struct Base {
  virtual void ok1() throw(int, float);
  virtual void ok2() throw(A);
  virtual void ok3() throw(A, int);
  virtual void ok4() throw(C *);
  virtual void ok5() throw(const C *);
  virtual void ok6() throw(void *);
  virtual void ok7() throw(int *);
  virtual void ok8() throw(C *const &);
  virtual void ok9() throw(int);
  virtual void ok10() throw(int);

  virtual void g1() throw();
  virtual void g2() throw(int);
  virtual void g3() throw(A);
  virtual void g4() throw(B1);
  virtual void g5() throw(A);
  virtual void g6() throw(C *&);
  virtual void g7() throw(const C *);
  virtual void g8() noexcept;
  virtual void g9() throw(int);
};

struct Derived : Base {
  void ok1() throw(float);
  void ok2() throw(B1, B2);
  void ok3() throw(int &, const A &);
  void ok4() throw(E *);
  void ok5() throw(C *);
  void ok6() throw(E *);
  void ok7() throw(nullptr_t);
  void ok8() throw(E *);
  void ok9() throw();
  void ok10() noexcept;

  void g1() throw(int);
  void g2();
  void g3() throw(D);
  void g4() throw(A);
  void g5() throw(P);
  void g6() throw(E *);
  void g7() throw(volatile C *);
  void g8() throw(int);
  void g9() noexcept(false);
};
