// [meta.const.eval]/3 (P2641R4), [class.base.init]/13.1, [basic.life]/1,
// [expr.const]/10.8 (P3533R2): a complete-object constructor initializes
// its virtual bases first, in order; while one is being initialized the
// later virtual bases and every non-virtual subobject have not begun their
// lifetime, and after them the virtual bases have. A base-subobject
// constructor does not initialize them: they are within their lifetime.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ requires: clang>=22
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1 has no __builtin_is_within_lifetime
#define L __builtin_is_within_lifetime
struct VB { int v; constexpr VB(int x) : v(x) {} };
struct VD : virtual VB {
  int w;
  consteval VD() : VB(L(&w) ? 5 : 6), w(L(&v)) {}
};
static_assert(VD().v == 6);
static_assert(VD().w == 1);
struct V1 { int a; };
struct V2 { int b; };
struct VV : virtual V1, virtual V2 {
  int c;
  consteval VV() : V1{L(&b) * 10 + L(&c)}, V2{L(&a)}, c(L(&a) + L(&b)) {}
};
static_assert(VV().a == 0 && VV().b == 1 && VV().c == 2);
struct VB2 { int q; };
struct M2 : virtual VB2 {
  bool res;
  consteval M2(const int* later) : VB2{1}, res(!L(later) && L(&q)) {}
};
struct VX : M2 { int z; consteval VX() : VB2{2}, M2(&z), z(3) {} };
static_assert(VX().res && VX().q == 2);
