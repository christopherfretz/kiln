// [class.virtual]/2: an overrider with the same ref-qualifier as the base
// virtual function overrides it; a ref-qualified function hiding a
// non-virtual base function, or one with a different parameter-type-list,
// is valid.
//@ std: c++20
//@ expect: compile-ok
struct B {
  virtual int f() & { return 1; }
  virtual int g() && { return 2; }
  virtual int h() { return 3; }
  int m() { return 4; }
  virtual int p(int) { return 5; }
};
struct D : B {
  int f() & override { return 10; }
  int g() && override { return 20; }
  int h() override { return 30; }
  int m() & { return 40; }
  int p() & { return 50; }
};
int use() {
  D d;
  B &b = d;
  return b.f() + D().g() + b.h() + d.m() + d.p() + d.B::p(0);
}
