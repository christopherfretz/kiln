// [class.inhctor.init]/1-2, [class.base.init]/7,13, [namespace.udecl]/3
// (C++26, P3533R2): constructors inherited by a class with a virtual base
// are constexpr. A constructor inherited from the virtual base itself
// initializes it from its arguments when the inheriting class is the most
// derived one; when a further-derived class initializes the virtual base
// (its own mem-initializer, or by default), the inherited constructor's
// initialization of it is skipped. Same values during constant evaluation
// and at run time; checked against clang 19's run time with `constexpr`
// removed (no clang implements P3533). The C++23 rejection is
// constexpr-vbase.cpp.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: run
//@ strict-diags
//@ stdout: 7291 7291
extern "C" int printf(const char *, ...);

struct V {
  int v;
  constexpr V(int x = 1) : v(x) {}
};
struct B : virtual V { using V::V; int b = 2; };
struct I : B { using B::B; int i = 3; };
struct J : I { constexpr J() : V(9), I(5) {} };
struct K : B { constexpr K() : B(4) {} };

constexpr int f(int seven) {
  I a(seven);
  J j;
  K k;
  return a.v * 1000 + a.b * 100 + j.v * 10 + k.v;
}
static_assert(f(7) == 7291);
constexpr int ct = f(7);

int main(int argc, char **) { printf("%d %d\n", ct, f(argc + 6)); }
