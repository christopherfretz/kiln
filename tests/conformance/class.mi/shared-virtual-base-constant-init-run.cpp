// [class.mi]/4, [dcl.constexpr]/3 (P3533R2, C++26): a class whose two
// direct bases share one virtual base has a constexpr implicit default
// constructor in C++26, so `x` is constant-initialized. kiln emits its
// constant image: `V` is nearly empty, so it is the primary virtual base
// of `M1` and `M2` (slot 0 of their types, holding the vptr), allocated
// once in `X` as its indirect primary base; `M2`'s slot holds `M2`'s own
// address point (WP-902). clang initializes it dynamically.
//@ expect: run
//@ stdout: 4 2 3
#include <cstdio>
struct V { virtual int v() { return 1; } };
struct M1 : virtual V { virtual int m1() { return 2; } };
struct M2 : virtual V { virtual int m2() { return 3; } };
struct X : M1, M2 { int v() override { return 4; } };
X x;
int main() {
  V* p = &x;
  std::printf("%d %d %d\n", p->v(), x.m1(), x.m2());
}
