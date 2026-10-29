// [class.member.lookup], [class.virtual] (clang -Woverloaded-virtual, in -Wall):
// a member function hiding a base virtual function of the same name with a
// different signature warns, with the reason (parameter type or qualifiers).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall
//@ diag: 17:8 warning "'B::g' hides overloaded virtual function"
//@ diag: 12:16 note "hidden overloaded virtual function 'A::g' declared here: type mismatch at 1st parameter ('int' vs 'long')"
//@ diag: 18:8 warning "'B::h' hides overloaded virtual function"
//@ diag: 13:16 note "hidden overloaded virtual function 'A::h' declared here: different qualifiers ('const' vs unqualified)"
struct A {
  virtual void g(int);
  virtual void h() const;
  virtual void k(int);
};
struct B : A {
  void g(long);
  void h();
  void k(int) override;
};
