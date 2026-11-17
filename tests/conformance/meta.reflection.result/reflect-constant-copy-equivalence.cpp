// [meta.reflection.result]/7-8, [temp.arg.nontype]/4 (P2996R13):
// reflect_constant(V) of a class type throws when the template parameter
// object copy-initialized from V would not be template-argument-equivalent
// to V (a copy constructor that changes the value): `constexpr T v = V;`
// copies V, the object is copied from v and must be equivalent to v, and
// has v's value. A copy that changes a pointer to a subobject, or makes
// another union member active, likewise. Found running GCC's C++
// testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 throws for every class with a user-provided copy constructor, an equivalent copy too
//@ expect: compile-ok
#include <meta>
struct A {
  int x;
  constexpr A(int p) : x(p) {}
  constexpr A(const A &o) : x(o.x ? o.x - 1 : o.x) {}
};
consteval bool ok(int x) {
  try {
    (void)std::meta::reflect_constant(A(x));
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
static_assert(ok(0));
static_assert(ok(1));
static_assert(!ok(2));
static_assert(!ok(4));
static_assert([:std::meta::reflect_constant(A(1)):].x == 0);
static_assert([:std::meta::reflect_constant(A(0)):].x == 0);

struct G { int a[2]; int c; };
constexpr G g{};
struct P {
  const int *p;
  constexpr P(const int *q) : p(q) {}
  constexpr P(const P &o)
      : p(o.p == g.a ? g.a + 1 : o.p == g.a + 1 ? g.a : o.p) {}
};
consteval bool okp(const int *q) {
  try { (void)std::meta::reflect_constant(P(q)); } catch (std::meta::exception &) { return false; }
  return true;
}
static_assert(!okp(g.a));
static_assert(!okp(g.a + 1));
static_assert(okp(&g.c));
static_assert([:std::meta::reflect_constant(P(&g.c)):].p == &g.c);

union U {
  int a;
  int b;
  constexpr U(int v) : a(v) {}
  constexpr U(const U &o) : b(o.a) {}
};
consteval bool oku() {
  try { (void)std::meta::reflect_constant(U(3)); } catch (std::meta::exception &) { return false; }
  return true;
}
static_assert(!oku());

struct T { int x; };
static_assert([:std::meta::reflect_constant(T{5}):].x == 5);
