// [class.copy.assign]/12, [class.copy.ctor]/14, [basic.types.general]/4:
// a trivial copy of a base subobject copies the base, not the derived
// members laid out in its tail padding (Itanium dsize < sizeof), in
// constant evaluation and at run time; so does a typed `memmove` in
// constant evaluation (at run time it copies bytes).
// libc++ test std/algorithms/alg.modifying.operations/alg.copy/
// copy_backward.pass.cpp:83 (PaddedBase, WP-276). Verified against clang 19.
//@ expect: run
//@ stdout: 6 61 6 6
#include <cstdio>
struct PB {
  constexpr PB(short a, signed char b) : a_(a), b_(b) {}
  short a_;
  signed char b_;
};
struct D : PB {
  constexpr D(short a, signed char b, signed char c) : PB(a, b), c_(c) {}
  signed char c_;
};
static_assert(sizeof(D) == sizeof(PB));
constexpr int assign() { D s(1, 2, 3), d(4, 5, 6); PB &r = d; r = s; return d.c_; }
constexpr int via_copy() {
  D s(1, 2, 3), d(4, 5, 6);
  PB tmp = s;
  static_cast<PB &>(d) = tmp;
  return d.c_ * 10 + d.a_;
}
constexpr int move_bytes() {
  D s(1, 2, 3), d(4, 5, 6);
  __builtin_memmove(static_cast<PB *>(&d), static_cast<PB *>(&s), sizeof(PB));
  return d.c_;
}
static_assert(assign() == 6 && via_copy() == 61 && move_bytes() == 6);
int main() {
  D s(1, 2, 3), d(4, 5, 6);
  static_cast<PB &>(d) = s;
  constexpr int m = move_bytes();
  std::printf("%d %d %d %d\n", assign(), via_copy(), m, d.c_);
}
