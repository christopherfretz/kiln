// Multiple inheritance: secondary vtable groups, this-adjusting thunks
// (_ZThn) for methods and the D1/D0 destructors, virtual delete via a
// secondary base. (Every function is noexcept: no EH before WP-100.)
struct A {
  int a = 1;
  virtual int fa() noexcept { return a; }
  virtual ~A() noexcept {}
};
struct B {
  int b = 2;
  virtual int fb() noexcept { return b; }
  virtual int g() noexcept { return 7; }
  virtual ~B() noexcept {}
};
struct C : A, B {
  int c = 3;
  int fb() noexcept override { return c + b; }
  int fa() noexcept override { return c + a; }
};
int via_b(B* p) noexcept { return p->fb() + p->g(); }
int test() noexcept {
  C c;
  return via_b(&c) + c.fa();
}
void kill(B* p) noexcept { delete p; }
