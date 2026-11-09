// [dcl.fct]/6, CWG2553: a function with an explicit object parameter
// cannot override a virtual function; clang reports it at `this` with a
// note at the overridden function (WP-1041). Allocation and deallocation
// functions are implicitly static, so they cannot take one either.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ diag: 24:10 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 19:16 note "overridden virtual function is here"
//@ diag: 25:9 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 20:15 note "overridden virtual function is here"
//@ diag: 26:10 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 21:16 note "overridden virtual function is here"
//@ diag: 29:10 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 19:16 note "overridden virtual function is here"
//@ diag: 32:22 error "an explicit object parameter cannot appear in a static function"
//@ diag: 33:24 error "an explicit object parameter cannot appear in a static function"
struct B {
  virtual void f(int);
  virtual int g() const;
  virtual void h(int, ...);
};
struct D : B {
  void f(this D&, int);
  int g(this const D&);
  void h(this D&, int, ...);
};
template<class T> struct E : B {
  void f(this E&, int);
};
struct A {
  void *operator new(this A&, decltype(sizeof 0));
  void operator delete(this A&, void *);
};
