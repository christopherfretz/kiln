// [class.virtual] (clang -Winconsistent-missing-override, on by default): once a
// class marks one overrider 'override', the overriders not so marked warn; a
// class marking none stays quiet.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 17:8 warning "'g' overrides a member function but is not marked 'override'"
//@ diag: 12:16 note "overridden virtual function is here"
//@ diag: 18:16 warning "'h' overrides a member function but is not marked 'override'"
//@ diag: 13:16 note "overridden virtual function is here"
struct A {
  virtual void f();
  virtual void g();
  virtual void h();
};
struct B : A {
  void f() override;
  void g();
  virtual void h();
};
struct C : A {
  void f();
  void g();
};
