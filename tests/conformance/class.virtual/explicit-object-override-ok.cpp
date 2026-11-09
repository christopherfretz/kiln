// [dcl.fct]/6, CWG2553 valid twin: an explicit-object member function that
// does not match a base virtual function's parameters does not override it
// (it only hides it), so it is well-formed (WP-1041).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
struct B {
  virtual void f(int);
  virtual int g() const;
  void k(int);
};
struct D : B {
  void f(this D&, long);
  int g(this const D&, int);
  void k(this D&, int);
  static void *operator new(decltype(sizeof 0));
  void operator delete(void *);
};
template<class T> struct E : B {
  void f(this E&, T);
};
E<long> e;
