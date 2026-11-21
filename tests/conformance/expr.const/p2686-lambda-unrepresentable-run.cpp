//@ expect: run
//@ skip(clang): neither clang 22 nor GCC 16.1 implements P2686R5 constexpr references to automatic objects
//@ stdout: 2 2 2 5 5 2 2
// [expr.const]/9-10, [expr.const.init]/2-3 (C++26, P2686R5): the valid
// neighbours of p2686-lambda-unrepresentable-errors.cpp (WP-1831). A
// constexpr local pointing to an automatic object is usable in its own
// function, in a nested block, and unevaluated in a lambda body; in a
// lambda body it is captured (naming it odr-uses it) and read at run time
// through the capture. A constexpr variable without automatic referents
// (a constexpr object, a constexpr binding of one, a reference or pointer
// to a static object) stays usable in a lambda body.
extern "C" int printf(const char *, ...);
struct S { int x, y; };
constexpr S gs{1, 2};
int by_ref() {
  constexpr auto [a, b] = S{1, 2};
  constexpr const int *p = &b;
  static_assert(*p == 2);
  { static_assert(*p == 2); }
  return [&] {
    static_assert(sizeof(*p) == sizeof(int));
    return *p;
  }();
}
int binding_in_lambda() {
  constexpr auto [a, b] = S{1, 2};
  return [&] {
    static_assert(b == 2);
    return b;
  }();
}
int through_reference() {
  constexpr S cl{1, 2};
  constexpr auto &[a, b] = cl;
  static_assert(b == 2);
  return [&] { return b; }();
}
int by_copy() {
  constexpr int v = 5;
  constexpr const int *p = &v;
  return [=] { return *p; }();
}
int plain_constexpr() {
  constexpr int v = 5;
  constexpr S cl{1, 2};
  return [&] {
    static_assert(v == 5 && cl.y == 2);
    return v;
  }();
}
int static_referents() {
  constexpr const int &r = gs.y;
  constexpr const int *p = &gs.y;
  return [&] {
    static_assert(r == 2 && *p == 2);
    return r;
  }();
}
int main() {
  int r1 = by_ref();
  int r2 = binding_in_lambda();
  int r3 = through_reference();
  int r4 = by_copy();
  int r5 = plain_constexpr();
  int r6 = static_referents();
  printf("%d %d %d %d %d %d %d\n", r1, r2, r3, r4, r5, r6, gs.y);
}
