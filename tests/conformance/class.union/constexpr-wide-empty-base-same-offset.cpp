// [class.union.general]/6, [expr.const]/10.9, [intro.object]/9 (WP-642): in
// a class with enough members for the Oven's offset index (16 or more), a
// union member is activated through a nested subobject that starts at the
// same offset as the class's empty bases (which hold no storage), and a
// read of the inactive member is diagnosed naming the union member that
// holds it, not the empty base sharing its offset.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:10 note "read of member 'i' of union with active member 'f' is not allowed in a constant expression"
//@ diag: 48:15 note "in call to 'inactive()'"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:14 note "read of member 'f' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 55:15 note "in call to 'inactive_last()'"
struct E {};
struct E2 {};
union U {
  int i;
  float f;
};
struct In : E {
  U u;
};
struct S : E, E2 {
  In in;
  int a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15;
  In last;
};

constexpr int ok() {
  S s{};
  s.in.u.f = 2.0f;
  s.last.u.f = 4.0f;
  s.last.u.i = 3;
  E *e = &s;
  E *ie = &s.in;
  E2 *e2 = &s;
  return int(s.in.u.f) + s.last.u.i + (static_cast<S *>(e) == &s) +
         (static_cast<In *>(ie) == &s.in) + (static_cast<S *>(e2) == &s);
}
static_assert(ok() == 8);

constexpr int inactive() {
  S s{};
  s.in.u.f = 2.0f;
  return s.in.u.i;
}
static_assert(inactive() == 0);

constexpr int inactive_last() {
  S s{};
  s.last.u.i = 2;
  return int(s.last.u.f);
}
static_assert(inactive_last() == 0);
