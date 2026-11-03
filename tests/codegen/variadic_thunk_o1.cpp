// WP-462: `variadic_thunk.cpp` above -O0: the thunk of a variadic virtual
// function defined here is defined after it (a `musttail` forward); one of
// a method defined elsewhere stays a declaration in the vtable but gets a
// definition's `uwtable` (clang `maybeEmitThunk(ForVTable)`), a
// return-adjusting one too, and one of a method with an incomplete class
// parameter is the attribute-less `void ()` placeholder.
//@ args: -O1
struct I;
struct A { virtual int f(int, ...); int a; };
struct B {
  virtual int f(int, ...);
  virtual int g(int, ...);
  virtual B *h(int, ...);
  virtual int u(I);
  int b;
};
struct C : A, B {
  int f(int x, ...) override;
  int g(int, ...) override;
  C *h(int, ...) override;
  int u(I) override;
};
int C::f(int x, ...) { return x + 1; }
int use(C *c) { return c->g(1, 2.0); }
C *make() { return new C; }
