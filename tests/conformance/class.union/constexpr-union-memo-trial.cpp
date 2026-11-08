// [class.union.general]/2, [expr.const]/10.9 (clang extension
// __builtin_constant_p), kiln's trial evaluation (WP-760): a failed trial
// that made a member of a heap union active, and read it, is rolled back
// with the remembered checks of that read (spec/oven.md "Union-path
// memo"): the other member is active again, and the read of the trial's
// member after it is diagnosed. clang keeps the writes of a failed
// speculative evaluation (constant-p-trial-rollback.cpp).
//@ skip(clang): clang keeps the heap writes of a failed speculative evaluation
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:27 note "read of member 'l' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 37:15 note "in call to 'rollback(true)'"
union U {
  int a[4];
  long l;
};

constexpr int switch_fail(U* h) {
  h->l = 1;
  long r = h->l + h->l;
  int z = 0;
  return static_cast<int>(r) / z;
}

constexpr int rollback(bool reread) {
  U* h = new U{{1, 2, 3, 4}};
  int r = h->a[0] + h->a[3];
  r += __builtin_constant_p(switch_fail(h));
  r += h->a[1];
  if (reread)
    r += static_cast<int>(h->l);
  delete h;
  return r;
}
static_assert(rollback(false) == 7);
static_assert(rollback(true) == 7);
