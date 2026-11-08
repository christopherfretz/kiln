// [class.mi]/4, [dcl.constexpr]/3 (P3533R2, C++26): a class whose two
// direct bases share one virtual base has a constexpr implicit default
// constructor in C++26, so `x` is constant-initialized. kiln's constant
// image of such an object (one virtual-base subobject reached through two
// paths) is not emitted yet; clang initializes it dynamically.
//@ expect: run
//@ xfail(kiln): WP-873 follow-up, codegen of a constant-initialized object with a shared virtual base (consts.rs record_elems "class constants with this member")
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
