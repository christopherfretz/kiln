// [expr.ref]/7 (P2996R13): `E.[: r :]` with `r` a direct base class
// relationship (D, B) designates the direct base class subobject of type
// B of the D object, not the result of a search for B from E's class: a
// class with a direct `B` and an indirect one (through `C`) is not
// ambiguous here, E may be of a class derived from D, and `p->[: r :]`
// is `(*p).[: r :]`, the subobject (not a pointer to it). Found running
// GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain} -Wno-inaccessible-base
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 crashes converting an object of a class derived from D along a base relationship of D
//@ expect: run
#include <meta>
struct B { int x; };
struct C : B {};
struct D : B, C { int y; };
struct E : D {};
constexpr auto ctx = std::meta::access_context::unchecked();
constexpr auto b0 = std::meta::bases_of(^^D, ctx)[0];
constexpr auto b1 = std::meta::bases_of(^^D, ctx)[1];
constexpr int f() {
  D d{};
  d.[:b0:].x = 1;
  d.[:b1:].x = 2;
  D *p = &d;
  return p->[:b0:].x * 10 + static_cast<C &>(d).x;
}
static_assert(f() == 12);
constexpr int g() {
  E e{};
  e.[:b0:].x = 5;
  const E &r = e;
  return r.[:b0:].x;
}
static_assert(g() == 5);
int main() {
  D d{};
  d.[:b0:].x = 3;
  d.[:b1:].x = 4;
  E e{};
  e.[:b0:].x = 6;
  B &rb = d.[:b0:];
  return d.[:b0:].x * 10 + static_cast<C &>(d).x == 34 && rb.x == 3 && e.[:b0:].x == 6 ? 0 : 1;
}
