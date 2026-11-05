// [class.union.general]/5-6, [dcl.array]/6, [expr.const]/10.9 (WP-644): one
// cached path (Box, offset, int) leads into an array of unions whose
// elements have different active members; each element's active member is
// read from memory on every access, so a read of `i` in an element whose
// active member is `f` is diagnosed while its neighbour's `i` is fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:21 note "read of member 'i' of union with active member 'f' is not allowed in a constant expression"
//@ diag: 57:15 note "in call to 'wrong()'"
struct Half {
  short lo;
  short hi;
};
union V {
  int i;
  float f;
  Half h;
};
struct Box {
  V v[6];
};

constexpr int mixed() {
  Box b{};
  for (int k = 0; k < 6; ++k) {
    if (k % 3 == 0)
      b.v[k].i = k;
    else if (k % 3 == 1)
      b.v[k].f = static_cast<float>(k);
    else
      b.v[k].h.hi = static_cast<short>(k);
  }
  int s = 0;
  for (int k = 0; k < 6; ++k) {
    if (k % 3 == 0)
      s += b.v[k].i;
    else if (k % 3 == 1)
      s += static_cast<int>(b.v[k].f);
    else
      s += b.v[k].h.hi;
  }
  return s;
}
static_assert(mixed() == 15);

constexpr int wrong() {
  Box b{};
  for (int k = 0; k < 6; ++k) {
    if (k % 2)
      b.v[k].f = 1.0f;
    else
      b.v[k].i = k;
  }
  return b.v[2].i + b.v[3].i;
}
static_assert(wrong() == 2);
