// [class.copy.ctor]/6, [class.copy.ctor]/10, [class.copy.assign]/7,
// [class.default.ctor]/2, [module.interface], spec/modules.md §11.11
// "Implicit members" (WP-1292): the implicit special members of exported
// classes that need overload resolution over their bases' and members'
// (deleted, nontrivial, non-const-parameter and friend-accessible ones)
// are the ones of the replayed source; under `--bmi-image` they come from
// the declaration image.
//@ expect: run
//@ std: c++20
//@ module: imgim=Inputs/img-implicit.cppm
//@ stdout: 3 3 4 5 1
import imgim;
using namespace im;
extern "C" int printf(const char *, ...);
static_assert(!__is_trivially_copyable(D) && __is_constructible(D, const D &) &&
              __is_assignable(D &, D &&));
static_assert(!__is_constructible(X, const X &) && __is_constructible(X, X &&) &&
              !__is_assignable(X &, const X &) && __is_assignable(X &, X &&));
static_assert(__is_constructible(Y, const Y &) && !__is_trivially_constructible(Y, const Y &) &&
              __is_constructible(Y, Y &&));
static_assert(__is_constructible(W, W &&) && !__is_trivially_copyable(W) &&
              __is_constructible(W, const W &));
static_assert(!__is_assignable(U &, const U &) && __is_constructible(U, const U &));
static_assert(!__is_constructible(VV, const VV &) && __is_constructible(VV, VV &));
int main() {
  D a;
  D b = a;
  b = static_cast<D &&>(a);
  X x;
  X x2 = static_cast<X &&>(x);
  x2 = static_cast<X &&>(x);
  Y y;
  Y y2 = y;
  W w{};
  W w2 = static_cast<W &&>(w);
  W w3 = w2;
  U u;
  U u2 = u;
  Q *q = new Q;
  (void)q;
  VV v;
  VV v2 = v;
  printf("%d %d %d %d %d\n", b.b + b.d, x2.x, y2.c.c, u2.t.t, (int)sizeof(v2));
  return 0;
}
