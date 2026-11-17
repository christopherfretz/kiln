// [bit.cast]/2: the "Constant When" list excludes unions, pointers,
// pointer-to-members, volatile types and reference members, not classes
// with base classes. Base subobjects are bit-cast as members, in both
// directions; an empty base takes no bytes, and a derived member placed in
// a base's tail padding is copied from there.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
struct Empty {};
struct B1 { int a; };
struct B2 { short b, c; };
struct D : B1, Empty, B2 { int d; };
struct Flat { int a; short b, c; int d; };
static_assert(sizeof(D) == sizeof(Flat));

constexpr D dv = __builtin_bit_cast(D, Flat{1, 2, 3, 4});
static_assert(dv.a == 1 && dv.b == 2 && dv.c == 3 && dv.d == 4);
constexpr Flat fv = __builtin_bit_cast(Flat, dv);
static_assert(fv.a == 1 && fv.b == 2 && fv.c == 3 && fv.d == 4);

// Two levels, through an intermediate base.
struct Mid : B1 { int m; };
struct Leaf : Mid { int l; };
struct Three { int x, y, z; };
constexpr Leaf lv = __builtin_bit_cast(Leaf, Three{7, 8, 9});
static_assert(lv.a == 7 && lv.m == 8 && lv.l == 9);
static_assert(__builtin_bit_cast(Three, lv).z == 9);

// A base that is not POD for layout purposes lends its tail padding.
class T {
  int x = 0;
  char y = 0;

public:
  constexpr T() = default;
  constexpr T(int x, char y) : x(x), y(y) {}
  constexpr int get() const { return x + y; }
};
struct U : T {
  char z;
};
struct Bytes { unsigned char b[sizeof(U)]; };
constexpr U make() {
  U u;
  static_cast<T &>(u) = T(0x01020304, 5);
  u.z = 6;
  return u;
}
// The result holds indeterminate padding bytes, so it is no constexpr
// variable's value; the bytes that are values are read inside.
constexpr int sum() {
  Bytes b = __builtin_bit_cast(Bytes, make());
  int s = 0;
  for (int i = 0; i < 6; ++i)
    s += b.b[i];
  return s;
}
static_assert(sum() == 1 + 2 + 3 + 4 + 5 + 6);

// A class with bases as the source and as the destination of one cast.
struct E1 { unsigned short h; };
struct E2 { unsigned short k; };
struct P : E1, E2 {};
struct Q : E2, E1 {};
constexpr Q q = __builtin_bit_cast(Q, P{{1}, {2}});
static_assert(q.k == 1 && q.h == 2);
