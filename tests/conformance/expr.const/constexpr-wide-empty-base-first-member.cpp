// [intro.object]/9, [class.derived.general]/5, [expr.static.cast]/11,
// [expr.const]/10 (WP-642): an empty base sits at offset 0 under a
// non-empty first member of a class wide enough for the Oven's offset index
// (16 or more fields). The base and the member are distinct subobjects at
// the same address: casts through the base, through void* and back reach
// the complete object, and stores through the member are seen through the
// base-derived path.
//@ expect: compile-ok
struct E {
  constexpr int tag() const { return 7; }
};
struct E2 {};
struct S : E, E2 {
  int first;
  int m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, m15;
  int last;
};

constexpr int via_base() {
  S s{};
  E *e = &s;
  E2 *e2 = &s;
  S *back = static_cast<S *>(e);
  back->first = 11;
  static_cast<S *>(e2)->last = 13;
  return s.first + s.last + e->tag();
}
static_assert(via_base() == 31);

constexpr bool same_address() {
  S s{};
  void *pb = static_cast<E *>(&s);
  void *pm = &s.first;
  void *ps = &s;
  return pb == ps && pm == ps;
}
static_assert(same_address());

constexpr int via_void() {
  S s{};
  void *v = &s;
  S *p = static_cast<S *>(v);
  p->first = 3;
  int *f = static_cast<int *>(static_cast<void *>(&p->first));
  *f += 4;
  E *e = p;
  return static_cast<S *>(e)->first;
}
static_assert(via_void() == 7);

constexpr int member_walk() {
  S s{};
  int *q = &s.first;
  *q = 1;
  s.m15 = 2;
  s.last = 4;
  return s.first + s.m15 + s.last;
}
static_assert(member_walk() == 7);
