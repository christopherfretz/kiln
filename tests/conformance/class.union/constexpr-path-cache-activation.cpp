// [class.union.general]/5-6, [expr.const]/10.9 (WP-644): the Oven caches
// the type-level part of each walk from an allocation's root to a
// subobject per (record, offset, type), but never the active member. The
// same (S, offset, int) path is walked first through the inactive member
// `p` (the assignment `s.u.p.b = 2` while `l` is active makes `p` active),
// then after activation, then after `l` is made active again: the last
// read of `p.b` is diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:14 note "read of member 'p' of union with active member 'l' is not allowed in a constant expression"
//@ diag: 44:15 note "in call to 'reread()'"
struct P {
  int a;
  int b;
};
union U {
  long l;
  P p;
};
struct S {
  int pad;
  U u;
};

constexpr int activate() {
  S s{};
  s.u.l = 1;
  s.u.p.b = 2;
  return s.u.p.b;
}
static_assert(activate() == 2);

constexpr int reread() {
  S s{};
  int r = 0;
  for (int k = 0; k < 4; ++k) {
    s.u.p.b = k;
    r += s.u.p.b;
  }
  s.u.l = 3;
  return r + s.u.p.b;
}
static_assert(reread() == 6);

constexpr int back() {
  S s{};
  s.u.p.b = 1;
  s.u.l = 3;
  s.u.p.b = 4;
  return s.u.p.b + static_cast<int>(sizeof(s.u.l));
}
static_assert(back() == 12);
