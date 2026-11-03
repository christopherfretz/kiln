// [dcl.constinit], [class.mi], P3533: a constinit object of a class with
// virtual bases is a constant initializer (no dynamic initialization):
// each subobject's vptr points into the complete object's vtable group for
// it, and the virtual base is reached through the vbase offset the vtable
// holds. clang has no P3533, so kiln alone checks it.
//@ expect: run
//@ args: -std=c++26
//@ skip(clang): clang does not implement P3533 constexpr virtual bases
//@ stdout: 1 2 3 4 104 6 5 6
#include <cstdio>
struct V { virtual int f() const { return v; } int v = 1; };
struct B : virtual V { int b = 2; constexpr B() = default; };
struct C : virtual V { int c = 3; constexpr C() = default; };
struct D : B, C {
  int d = 4;
  constexpr D() = default;
  int f() const override { return 100 + d; }
};
struct N { int x; };
struct W : virtual N { int w = 5; constexpr W() : N{6} {} };
constinit D dd;
constinit W ww;
int main() {
  const C &c = dd;
  const V &v = c;
  const N &n = ww;
  std::printf("%d %d %d %d %d %d %d %d\n", v.v, dd.b, c.c, dd.d, v.f(),
              n.x, ww.w, static_cast<const N &>(ww).x);
}
