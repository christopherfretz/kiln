// [class.union.general]/5-6, [expr.const]/10.9 (WP-644): the path cache
// holds at most `PATH_CACHE_MAX` (crates/kiln_oven/src/ty/path.rs, 1 << 14)
// entries per map and clears when full. `Big` holds 40000 unions, so the
// walks below touch 40000 distinct (Big, offset, type) paths, more than
// twice the cap; the union checks are the same before and after each
// clear.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:11 note "read of member 'i' of union with active member 'f' is not allowed in a constant expression"
//@ diag: 47:15 note "in call to 'evicted()'"
union W {
  int i;
  float f;
};
struct Big {
  W w[40000];
};

constexpr int touch() {
  Big* b = new Big{};
  for (int k = 0; k < 40000; ++k) {
    if (k % 2)
      b->w[k].f = 1.0f;
    else
      b->w[k].i = 1;
  }
  int s = 0;
  for (int k = 0; k < 40000; ++k)
    s += k % 2 ? static_cast<int>(b->w[k].f) : b->w[k].i;
  b->w[0].f = 2.0f;
  s += static_cast<int>(b->w[0].f);
  delete b;
  return s;
}
static_assert(touch() == 40002);

constexpr int evicted() {
  Big* b = new Big{};
  for (int k = 0; k < 40000; ++k)
    b->w[k].i = k;
  b->w[1].f = 1.0f;
  int r = b->w[1].i;
  delete b;
  return r;
}
static_assert(evicted() == 1);
