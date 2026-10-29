// [class.abstract]/2-3: pointers and references to an abstract class, sizeof,
// 'this', and a definition of a pure virtual function are all fine.
//@ expect: compile-ok
//@ strict-diags
struct A {
  virtual void f() = 0;
};
struct D : A {
  void f() override;
};
void f() {
  (void)sizeof(A);
  static D d;
  A *p = &d;
  (void)p;
}
struct H {
  virtual void m() = 0;
  H *self() { return this; }
  void use(H &);
};
void H::m() {}
