// [class.union.general]/5-6, [expr.const]/10.9 (WP-644): an `int` at the
// same byte offset of two different root types, a plain member of `B` and
// a member of a union in `A`. The path cache keys on the root type, so the
// union on `A`'s path is checked even after `B`'s path was cached (and
// `B`'s stays free of union checks after `A`'s).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:10 note "read of member 'i' of union with active member 'f' is not allowed in a constant expression"
//@ diag: 38:15 note "in call to 'in_union()'"
union U {
  int i;
  float f;
};
struct A {
  long x;
  U u;
};
struct B {
  long x;
  int i;
  float f;
};
static_assert(__builtin_offsetof(A, u) == __builtin_offsetof(B, i));

constexpr int plain() {
  B b{};
  b.f = 2.0f;
  return b.i + static_cast<int>(b.f);
}
static_assert(plain() == 2);

constexpr int in_union() {
  A a{};
  a.u.f = 2.0f;
  return a.u.i;
}
static_assert(in_union() == 0);

constexpr int plain_again() {
  B b{};
  b.i = 5;
  return b.i;
}
static_assert(plain_again() == 5);
