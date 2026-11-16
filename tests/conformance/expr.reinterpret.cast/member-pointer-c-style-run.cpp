// [expr.cast]/4: a C-style cast between pointer-to-member types that
// static_cast cannot do is a reinterpret_cast ([expr.reinterpret.cast]/10),
// with a const_cast after it if needed; the bits are kept, so a null member
// pointer stays null and round trips give the original value. Found running
// GCC's C++ testsuite (WP-1467).
//@ expect: run
//@ stdout: null 1
//@ stdout: val 7
//@ stdout: back 7
//@ stdout: rt 9
//@ stdout: g 5
//@ stdout: virt 3
//@ stdout: virt2 3
//@ stdout: f 1
//@ stdout: nf 1
//@ stdout: const 7
#include <cstdio>
struct P { long pad[3]; };
struct A {
  int a; long l;
  int f() { return 1; }
  virtual int v() { return 2; }
  int g(int x) { return x; }
};
struct B : P, A { int b; int v() override { return 3; } };
typedef int A::*aip;
typedef long A::*alp;
typedef int (A::*F0)();
typedef int (A::*F1)(int);
typedef void (A::*FV)();
int main() {
  // A null data member pointer stays null through a reinterpreting cast.
  aip n = (aip)(alp)0;
  std::printf("null %d\n", n == 0);
  // static_cast steps (derived-to-base adjustments) still apply.
  int B::*d = (int B::*)&A::a;
  B b; b.a = 7; b.l = 9;
  std::printf("val %d\n", b.*d);
  aip back = (aip)d;
  std::printf("back %d\n", b.*back);
  // Reinterpreting round trips keep the value ([expr.reinterpret.cast]/10).
  alp r = (alp)(aip)&A::l;
  std::printf("rt %ld\n", b.*r);
  F1 g1 = (F1)(F0)&A::g;
  std::printf("g %d\n", (b.*g1)(5));
  int (B::*vb)() = (int (B::*)())&A::v;
  std::printf("virt %d\n", (b.*vb)());
  F0 vv = (F0)vb;
  std::printf("virt2 %d\n", (b.*vv)());
  F0 f0 = (F0)(FV)(F0)&A::f;
  std::printf("f %d\n", (b.*f0)());
  F0 nf = (F0)(F1)0;
  std::printf("nf %d\n", nf == 0);
  // const_cast part: a pointer to const data member.
  const int A::*ca = &A::a;
  aip m = (aip)ca;
  std::printf("const %d\n", b.*m);
}
