// [class.bit]/1, [class.union.general]/6, [expr.const]/10.9 (WP-642): in a
// class wide enough for the Oven's offset index (16 or more fields), several
// bit-fields share one allocation unit. A store to one member leaves its
// siblings in the unit intact; a union whose members are bit-field structs
// switches its active member by assignment through a bit-field, and a read
// of a bit-field of the inactive member is diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:10 note "read of member 'x' of union with active member 'y' is not allowed in a constant expression"
//@ diag: 66:15 note "in call to 'read_inactive()'"
struct Bits {
  unsigned a : 3;
  unsigned b : 5;
  unsigned c : 7;
  unsigned d : 1;
  unsigned e : 16;
};
union BU {
  Bits x;
  struct {
    unsigned lo : 4;
    unsigned hi : 4;
  } y;
};
struct W {
  int f0, f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11, f12, f13, f14, f15;
  Bits bits;
  unsigned p : 2, q : 2, r : 4;
  BU bu;
};

constexpr unsigned siblings() {
  W w{};
  w.bits.a = 5;
  w.bits.b = 17;
  w.bits.c = 99;
  w.bits.d = 1;
  w.bits.e = 40000;
  w.bits.b = 3;
  w.p = 1;
  w.q = 2;
  w.r = 9;
  w.q = 3;
  w.f15 = 7;
  return w.bits.a + w.bits.b + w.bits.c + w.bits.d + w.bits.e + w.p + w.q +
         w.r + w.f15;
}
static_assert(siblings() == 5 + 3 + 99 + 1 + 40000 + 1 + 3 + 9 + 7);

constexpr unsigned switch_active() {
  W w{};
  w.bu.x.c = 4;
  w.bu.y = {};
  w.bu.y.hi = 6;
  w.bu.y.lo = 2;
  return w.bu.y.hi * 16 + w.bu.y.lo;
}
static_assert(switch_active() == 98);

constexpr unsigned read_inactive() {
  W w{};
  w.bu.y.hi = 6;
  return w.bu.x.b;
}
static_assert(read_inactive() == 0);
