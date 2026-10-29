// [class.static.mfct]/2, [class.virtual]/2: a static member function cannot
// override (or be) a virtual function of a base class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "'static' member function 'f' overrides a virtual function in a base class"
//@ diag: 10:16 note "overridden virtual function is here"
//@ diag: 15:14 error "'static' member function 'g' overrides a virtual function in a base class"
//@ diag: 11:15 note "overridden virtual function is here"
struct A {
  virtual void f();
  virtual int g(int);
};
struct B : A {
  static void f();
  static int g(int) { return 0; }
};
