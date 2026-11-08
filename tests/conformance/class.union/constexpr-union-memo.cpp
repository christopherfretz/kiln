// [class.union.general]/2, [expr.const]/10.9 (WP-760): the Oven remembers
// a successful active-member check per (allocation, type, offset), or for
// every element of an array member at once, and forgets every entry when
// any active member of the evaluation changes (spec/oven.md "Union-path
// memo"). Each read below follows reads of the same objects that were
// checked and remembered; after the change it is diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:14 note "read of member 'a' of union with active member 'l' is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'region(true)'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:31 note "read of member 'l' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 54:15 note "in call to 'reverse(true)'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:14 note "read of member 'a' of union with active member 'l' is not allowed in a constant expression"
//@ diag: 74:15 note "in call to 'per_union(true)'"
union U {
  int a[8];
  long l;
};
struct S {
  int pad;
  U u;
};

// A whole-array entry, then the active member changes.
constexpr int region(bool change) {
  S s{};
  int r = 0;
  for (int k = 0; k < 8; ++k)
    s.u.a[k] = k;
  for (int k = 0; k < 8; ++k)
    r += s.u.a[k];
  if (change)
    s.u.l = 1;
  return r + s.u.a[3];
}
static_assert(region(false) == 31);
static_assert(region(true) == 31);

// A single-offset entry, then the union is assigned anew.
constexpr int reverse(bool change) {
  S s{};
  s.u.l = 2;
  long r = 0;
  for (int k = 0; k < 4; ++k)
    r += s.u.l;
  if (change)
    s.u = U{};
  return static_cast<int>(r + s.u.l);
}
static_assert(reverse(false) == 10);
static_assert(reverse(true) == 10);

// Unions in one allocation: the change of one forgets the entries of all,
// and the others keep their active members.
struct E {
  int x;
  U u;
};
constexpr int per_union(bool change) {
  E e[4]{};
  int r = 0;
  for (int i = 0; i < 4; ++i)
    for (int k = 0; k < 8; ++k)
      r += e[i].u.a[k] + 1;
  if (change)
    e[2].u.l = 1;
  r += e[1].u.a[1] + e[3].u.a[7];
  return r + e[2].u.a[1];
}
static_assert(per_union(false) == 32);
static_assert(per_union(true) == 32);
