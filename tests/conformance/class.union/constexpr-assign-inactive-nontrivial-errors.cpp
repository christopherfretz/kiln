// [class.union.general]/6, [expr.const]/10.8: an assignment through an
// inactive union member whose class has a non-trivial default constructor
// (a user-provided one, a default member initializer, a member that has
// one) does not begin its lifetime: the assignment is to an object
// outside its lifetime, also for arrays of such classes and when the
// failing member is an intermediate one of the chain. kiln accepted these
// and made the member active. Notes as clang 22's; GCC 16.1 rejects them
// too. The accepted side is constexpr-assign-inactive-trivial.cpp.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:10 note "assignment to member 'nt' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 56:15 note "in call to 'nontrivial()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:10 note "assignment to member 'in' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'member_initializer()'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:14 note "assignment to member 'arr' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 68:15 note "in call to 'array()'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:13 note "assignment to member 'dnt' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 74:15 note "in call to 'chain()'"
//@ diag: 80:15 error "static assertion expression is not an integral constant expression"
//@ diag: 77:16 note "assignment to member 'nt' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 80:15 note "in call to 'nested()'"
//@ diag: 99:15 error "static assertion expression is not an integral constant expression"
//@ diag: 96:15 note "assignment to member 'm' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 99:15 note "in call to 'intermediate()'"
struct NT {
  int x;
  constexpr NT() : x(0) {}
};
struct Init {
  int x = 0;
};
struct DeepNT {
  NT t;
};
union U {
  int i;
  NT nt;
  Init in;
  NT arr[2];
  DeepNT dnt;
  constexpr U() : i(0) {}
};
struct N {
  U inner;
};
constexpr int nontrivial() {
  U u;
  u.nt.x = 1;
  return u.nt.x;
}
static_assert(nontrivial() == 1);
constexpr int member_initializer() {
  U u;
  u.in.x = 1;
  return u.in.x;
}
static_assert(member_initializer() == 1);
constexpr int array() {
  U u;
  u.arr[1].x = 1;
  return u.arr[1].x;
}
static_assert(array() == 1);
constexpr int chain() {
  U u;
  u.dnt.t.x = 1;
  return u.dnt.t.x;
}
static_assert(chain() == 1);
constexpr int nested() {
  N n{};
  n.inner.nt.x = 1;
  return n.inner.nt.x;
}
static_assert(nested() == 1);
// An intermediate member of the chain fails: `m` has a non-trivial
// default constructor (its union member does).
struct Mid {
  union {
    int j;
    NT nt;
  } in;
};
union W {
  int i;
  Mid m;
  constexpr W() : i(0) {}
};
constexpr int intermediate() {
  W w;
  w.m.in.nt.x = 1;
  return w.m.in.nt.x;
}
static_assert(intermediate() == 1);
