// [expr.new]/8: the size of an array new-expression may be a wide
// `_BitInt(N)` (N > 128); a constant-evaluated one allocates that many
// elements. x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok

using S = _BitInt(200);
constexpr int f() {
  int *p = new int[S(3)];
  p[2] = 7;
  int r = p[2];
  delete[] p;
  return r;
}
static_assert(f() == 7);
