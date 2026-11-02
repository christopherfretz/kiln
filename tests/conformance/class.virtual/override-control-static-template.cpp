// [class.virtual]/2,5, [class.static.mfct]/2: override control (clang
// CheckOverrideControl). A static member function that overrides a virtual one
// keeps its 'override'/'final' (the static mismatch is the error); a
// specifier deferred to instantiation (dependent base or type) is reported
// at the specifier, not the name; both specifiers on a non-virtual function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:18 error "only virtual member functions can be marked 'override'"
//@ diag: 44:12 error "only virtual member functions can be marked 'final'"
//@ diag: 45:3 error "'virtual' can only appear on non-static member functions"
//@ diag: 45:27 error "only virtual member functions can be marked 'override'"
//@ diag: 46:12 error "only virtual member functions can be marked 'override'"
//@ diag: 46:8 error "'i' is not virtual and cannot be declared pure"
//@ diag: 47:16 error "'j' marked 'override' but does not override any member functions"
//@ diag: 48:13 error "only virtual member functions can be marked 'override'"
//@ diag: 49:12 error "non-virtual member function marked 'override' hides virtual member function"
//@ diag: 37:16 note "hidden overloaded virtual function 'B::y' declared here: different number of parameters (1 vs 0)"
//@ diag: 41:15 error "'static' member function 'v' overrides a virtual function in a base class"
//@ diag: 34:16 note "overridden virtual function is here"
//@ diag: 42:15 error "'static' member function 'w' overrides a virtual function in a base class"
//@ diag: 35:16 note "overridden virtual function is here"
//@ diag: 43:15 error "'static' member function 'x' overrides a virtual function in a base class"
//@ diag: 36:16 note "overridden virtual function is here"
//@ diag: 57:15 error "only virtual member functions can be marked 'override'"
//@ diag: 53:15 error "only virtual member functions can be marked 'override'"
//@ diag: 61:7 note "in instantiation of template class 'TD<B>' requested here"
//@ diag: 54:15 error "only virtual member functions can be marked 'final'"
//@ diag: 58:9 error "only virtual member functions can be marked 'override'"
//@ diag: 62:9 note "in instantiation of template class 'TE<int>' requested here"
//@ diag: 59:15 error "'static' member function 'w' overrides a virtual function in a base class"
//@ diag: 35:16 note "overridden virtual function is here"
//@ diag: 63:14 error "'override' specifier is not allowed outside a class definition"
struct B {
  virtual void v();
  virtual void w();
  virtual void x();
  virtual void y(int);
  void nv();
};
struct D : B {
  static void v() override;
  static void w() override final;
  static void x() final;
  void a() final override;
  virtual static void s() override;
  void i() override = 0;
  virtual void j() override;
  void nv() override;
  void y() override;
};
template<class T> struct TD : T {
  void v() override;
  void nope() override;
  void late() final;
};
template<class T> struct TE : B {
  void nope() override;
  T q() override;
  static void w() override;
};
TD<B> tdb;
TE<int> tei;
void free1() override;
