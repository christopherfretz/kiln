// [expr.add]/4.3, [expr.const]/10.8, [basic.compound]/3 (WP-642): in a
// class wide enough for the Oven's offset index (16 or more fields),
// pointers formed at or beyond the end of a member, of the last member and
// of the complete object do not designate a field; arithmetic past them and
// reads through them are diagnosed, never resolved to a neighbouring field.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:5 note "cannot refer to element 2 of non-array object in a constant expression"
//@ diag: 36:15 note "in call to 'past_member()'"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:10 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 43:15 note "in call to 'read_one_past_last()'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:13 note "cannot access field of pointer past the end of object"
//@ diag: 50:15 note "in call to 'read_one_past_object()'"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:6 note "assignment to dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 58:15 note "in call to 'store_one_past_union()'"
union U {
  int i;
  float f;
};
struct W {
  int f0, f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11, f12, f13, f14, f15;
  U u;
  int last;
};

constexpr int past_member() {
  W w{};
  int *p = &w.f3;
  p += 2;
  return *p;
}
static_assert(past_member() == 0);

constexpr int read_one_past_last() {
  W w{};
  int *p = &w.last + 1;
  return *p;
}
static_assert(read_one_past_last() == 0);

constexpr int read_one_past_object() {
  W ws[2]{};
  W *p = ws + 2;
  return p->f0;
}
static_assert(read_one_past_object() == 0);

constexpr int store_one_past_union() {
  W w{};
  int *p = &w.u.i + 1;
  *p = 1;
  return w.last;
}
static_assert(store_one_past_union() == 0);

constexpr int ok() {
  W w{};
  int *p = &w.f3 + 1;
  w.last = 5;
  return (p == &w.f3 + 1) + w.last;
}
static_assert(ok() == 6);
