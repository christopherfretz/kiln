// [temp.res.general]/6, [class.virtual]: in a templated class the override
// checks that depend on a template parameter wait for instantiation (clang
// `CheckOverrideControl`, `CheckOverridingFunctionReturnType`,
// `CheckCompletedCXXClass`): 'override'/'final' with a dependent base or a
// dependent function type, a dependent return type, a static "overrider",
// multiple final overriders and -Winconsistent-missing-override. A check that
// depends on nothing dependent still fires.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:52 error "non-virtual member function marked 'override' hides virtual member function"
//@ diag: 13:16 note "hidden overloaded virtual function 'B::f' declared here: different qualifiers (unqualified vs 'const')"
struct B {
  virtual void f();
  virtual int r();
  virtual B *cov();
  virtual void ov(int);
  virtual void g();
};
template <class T> struct D1 : T { void f() override; void g() final; };
template <class... Ts> struct D2 : Ts... { void f() override; };
template <class T> struct D3 : B { void ov(T) override; };
template <class T> struct D4 : B { T r(); };
template <class T> struct D5 : B { T *cov(); };
template <class T> struct D6 : B { static void f(); };
template <class T> struct D7 : B { void f() override; void g(); };
struct M1 : virtual B { void f() final; };
struct M2 : virtual B { void f() final; };
template <class T> struct D8 : M1, M2 {};
template <class T> struct D9 : B { virtual void f() override final; virtual T h(); };
template <class T> struct D10 : B { void f() const override; };
