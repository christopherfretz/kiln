// [class.virtual], [class.dtor]/14, [expr.delete]/3, [expr.mptr.oper]:
// virtual dispatch at run time through vtables (WP-080): single and
// multiple inheritance with this-adjusting thunks, covariant returns with
// return-adjusting thunks, virtual destruction through a secondary base
// via the deleting destructor, and pointers to virtual member functions.
//@ expect: run
//@ stdout: base 1 derived 2
//@ stdout: mi 13 10 7
//@ stdout: cov 1 9
//@ stdout: ~C ~B ~A
//@ stdout: pmf 2 13 7
//@ stdout: ~C ~B ~A
extern "C" int printf(const char *, ...);
struct Base {
  int x = 1;
  virtual int f() { return x; }
  virtual const char *name() { return "base"; }
  virtual ~Base() {}
};
struct Derived : Base {
  int y = 2;
  int f() override { return y; }
  const char *name() override { return "derived"; }
};
struct A { int a = 3; virtual int fa() { return a; } virtual ~A() { printf(" ~A\n"); } };
struct B { int b = 4; virtual int fb() { return b; } virtual int g() { return 7; } virtual ~B() { printf(" ~B"); } };
struct C : A, B {
  int c = 6;
  int fb() override { return c + b + a; }
  int fa() override { return c + b; }
  ~C() override { printf("~C"); }
};
struct X { long x = 9; virtual void fx() {} };
struct R { int r = 1; virtual R *self() { return this; } };
struct S : X, R { S *self() override { return this; } };
int show(Base *p) { return printf("%s %d", p->name(), p->f()); }
int main() {
  Base b;
  Derived d;
  show(&b);
  printf(" ");
  show(&d);
  printf("\n");
  C c;
  B *pb = &c;
  A *pa = &c;
  printf("mi %d %d %d\n", pb->fb(), pa->fa(), pb->g());
  S s;
  R *pr = &s;
  printf("cov %d %ld\n", pr->self()->r, s.self()->x);
  B *heap = new C;
  delete heap;
  int (Base::*pf)() = &Base::f;
  int (B::*pg)() = &B::fb;
  int (B::*ph)() = &B::g;
  printf("pmf %d %d %d\n", (d.*pf)(), (pb->*pg)(), (pb->*ph)());
}
