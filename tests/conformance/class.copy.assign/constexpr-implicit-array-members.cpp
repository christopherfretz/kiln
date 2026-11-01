// [class.copy.assign]/12, [expr.const]: an implicitly-defined copy
// assignment of a class with (multi-dimensional) array members of
// trivially-copyable type is usable in a constant expression; kiln lowers
// the array member copy to `__builtin_memcpy` of a whole-array pointer.
//@ std: c++26
struct P { int x; };
struct S { char pad[0]; char c[2]; int m[2][3]; P p[2]; };

constexpr int f() {
  S a{}, b{};
  b.c[1] = 3;
  b.m[1][2] = 7;
  b.p[1].x = 4;
  a = b;
  return a.c[1] * 100 + a.m[1][2] * 10 + a.p[1].x;
}
static_assert(f() == 374);

constexpr int g() {
  const S b{};
  S a{};
  a.m[0][0] = 9;
  a = b;
  return a.m[0][0];
}
static_assert(g() == 0);
