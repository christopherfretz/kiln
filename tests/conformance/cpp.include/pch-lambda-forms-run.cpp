// [expr.prim.lambda], [expr.prim.lambda.closure], [expr.prim.lambda.capture],
// [class.local], [dcl.enum]: a header's lambdas (captures by copy and by
// reference, init-captures, `this` and `*this`, generic and pack lambdas,
// the conversion to a function pointer, lambdas in default member
// initializers, default arguments and namespace-scope initializers) and
// its local classes and enumerations behave the same through a
// precompiled header, and a closure type is the same type in the header
// and the unit. With `--pch-diff`, the header's snapshot carries them
// (WP-1350).
//@ expect: run
#include "Inputs/lambda_forms.h"

using namespace lf;

static_assert(__is_same(decltype(v1), V1));
static_assert(nsl(1) == 101 && nsg(1) == 2);

int main() {
  int a = 1, b = 2;
  long c = 3;
  auto local = [] { return 3; };
  int r = plain(1) + byref(1) + noncap() + initcap(1) + S{}.get() + generic() + use_tl() +
          int(tl(3L)) + tl(2) + v1() + withcap() + M{}.fp(3) + K::sm(1) + K{}.m + vt<int>(1) +
          int(vt<double>(1.5)) + defarg() + fp()(1) + use_tc() + TC<char>{}.f() + sum_each(a, b, c) +
          count_each<char, short>() + caps(1, 2, 3)() + initpack(2, 3)() + local();
  int l = lc1(1) + le1(1) + lc2() + lct(3) + int(lct(1.5)) + lvirt() + lboth(4);
  if (r != 2 + 6 + 10 + 11 + 6 + 7 + 5 + 10 + 5 + 10 + 4 + 2 + 2 + 5 + 2 + 2 + 6 + 7 + 5 + 2 + 6 + 3 + 6 + 6 + 3)
    return 1;
  return l == 3 + 7 + 12 + 6 + 3 + 2 + 8 ? 0 : 2;
}
