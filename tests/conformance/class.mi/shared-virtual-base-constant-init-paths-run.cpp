// [class.mi]/4, [dcl.constexpr]/3 (P3533R2, C++26): constant-initialized
// objects whose virtual bases are reached through several paths. `m1`: a
// primary virtual base in the complete object. `y`: a virtual base with
// data (not nearly empty, so never primary) gets its own slot after the
// non-virtual parts. `z`: two virtual bases whose own primary virtual base
// `V` is shared (slot 0 of a virtual base laid out as a base, WP-902).
// A wrong vptr or offset in kiln's constant image is a wrong dispatch or
// a wrong field read here; clang initializes these dynamically.
//@ expect: run
//@ stdout: 1 2
//@ stdout: 14 7 8 5 5
//@ stdout: 1 10 3 10
#include <cstdio>
struct V { virtual int v() { return 1; } };
struct M1 : virtual V { virtual int m1() { return 2; } };
struct M2 : virtual V { virtual int m2() { return 3; } };
struct W { int w = 5; virtual int v() { return 6; } };
struct N1 : virtual W { int n1 = 7; virtual int f1() { return n1; } };
struct N2 : virtual W { int n2 = 8; virtual int f2() { return n2; } };
struct Y : N1, N2 { int y = 9; int v() override { return w + y; } };
struct Z : virtual M1, virtual M2 { int m1() override { return 10; } };
M1 m1;
Y y;
Z z;
int main() {
  std::printf("%d %d\n", static_cast<V&>(m1).v(), m1.m1());
  W* q = &y;
  N2* r = &y;
  std::printf("%d %d %d %d %d\n", q->v(), y.f1(), r->f2(), y.w, static_cast<N2&>(y).w);
  V* s = &z;
  M2* t = &z;
  std::printf("%d %d %d %d\n", s->v(), z.m1(), t->m2(), static_cast<M1&>(z).m1());
}
