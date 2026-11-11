#ifndef UNNAMED_TAGS_H
#define UNNAMED_TAGS_H
// Unnamed tags no typedef names: the shape of <fenv.h>'s
// `enum { FE_INEXACT = ... };` (WP-1293).
enum { UT_A = 1, UT_B = 2 << 1, UT_C = UT_A | UT_B };
namespace ut {
enum : unsigned char { X = 7, Y };
inline int f(void) { return X + Y; }
}
struct UtS {
  enum { K = 3 };
  int v[K];
};
struct {
  int a;
  int f(void) const { return a + 1; }
} ut_g;
namespace ut {
struct {
  char c;
} v[2];
}
inline int ut_sum(void) { return UT_A + UT_B + UT_C + UtS::K + ut_g.f() + ut::v[1].c; }
#endif
