// [class.virtual]/2-3: each virtual function has a unique final overrider in
// every complete class; two overriders on sibling paths through a shared virtual
// base make the class ill-formed, unless the class overrides it itself, while
// non-virtual bases give separate subobjects with separate final overriders.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:8 error "virtual function 'V::f' has more than one final overrider in 'D'"
//@ diag: 12:16 note "overridden virtual function is here"
//@ diag: 16:8 note "final overrider of 'V::f' in 'L'"
//@ diag: 19:8 note "final overrider of 'V::f' in 'R'"
struct V {
  virtual void f();
  virtual void g();
};
struct L : virtual V {
  void f() override;
};
struct R : virtual V {
  void f() override;
  void g() override;
};
struct D : L, R {};
struct E : L, R {
  void f() override;
};
struct NL : V { void f() override; };
struct NR : V { void f() override; };
struct N : NL, NR {};
