// Pointers to virtual member functions: vtable offset + 1 (Itanium) or
// offset with adj|1 (ARM), through a secondary base, and the call path.
struct A { int a = 1; virtual int f(int x) noexcept { return a + x; } virtual int g() noexcept { return 2; } int h() noexcept { return 3; } };
struct B { long b = 5; virtual int k() noexcept { return 4; } };
struct C : B, A { int f(int x) noexcept override { return x * 2; } virtual int m() noexcept { return 7; } };
using PA = int (A::*)(int) noexcept;
using PC = int (C::*)() noexcept;
int callp(A* a, PA p) noexcept { return (a->*p)(3); }
int run() noexcept {
  C c;
  PA pf = &A::f;
  PC pg = &C::g;   // inherited from A via base B,A: adj
  PC pm = &C::m;
  PC pk = &C::k;
  int (A::*ph)() noexcept = &A::h;
  return callp(&c, pf) + (c.*pg)() + (c.*pm)() + (c.*pk)() + (c.*ph)() + (pf == &A::f);
}
constexpr PA gpa = &A::f;
PC gpc = &C::g;
