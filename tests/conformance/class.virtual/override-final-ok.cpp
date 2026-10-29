// [class.virtual]/2-4, [class.mem.general]: a function with the same name,
// parameter-type-list, cv- and ref-qualifiers as a base virtual member
// overrides it (and is virtual); 'override' and 'final' (also on classes) are
// accepted where they hold; one function can override the same base function in
// two subobjects (one reached through a virtual base).
//@ expect: compile-ok
//@ strict-diags
struct A {
  virtual int f();
  virtual A *clone();
  virtual void g() const;
  void h();
};
struct B : A {
  int f() override;
  B *clone() override;
  void g() const final;
  virtual void h();
};
struct C final : B {
  int f();
  C *clone();
};
struct D : virtual A {
  int f() override;
};
struct E : D, B {
  int f() override;
};
int use(C &c, A &a) { return c.f() + a.f(); }
