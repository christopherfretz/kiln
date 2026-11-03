// WP-462: `variadic_thunk_o1.cpp` before the LLVM passes: the thunk of the
// variadic method defined elsewhere is an `available_externally` musttail
// forward; the return-adjusting one is a declaration.
//@ args: -O1 -Xclang -disable-llvm-passes
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
