// [class.union.general]/6: an assignment `E1 = E2` begins the lifetime of
// each union member in S(E1): a member named by a member access on the
// left side, along the chain, of non-class type or of a class type with a
// non-deleted trivial default constructor, or an array of such. Accepted
// side: trivial members, arrays of them, chains `u.a.b.c = 1`, nested
// unions, a defaulted default constructor, and an assignment of the whole
// member. The rejected side is constexpr-assign-inactive-nontrivial-errors.cpp.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct TR {
  int x;
};
struct Deep {
  TR t;
};
struct Defaulted {
  int x;
  Defaulted() = default;
};
union U {
  int i;
  TR tr;
  TR arr[2];
  Deep deep;
  Defaulted d;
  constexpr U() : i(0) {}
};
struct N {
  U inner;
};
constexpr int trivial() {
  U u;
  u.tr.x = 1;
  return u.tr.x;
}
static_assert(trivial() == 1);
constexpr int array() {
  U u;
  u.arr[1].x = 2;
  return u.arr[1].x;
}
static_assert(array() == 2);
constexpr int chain() {
  U u;
  u.deep.t.x = 3;
  return u.deep.t.x;
}
static_assert(chain() == 3);
constexpr int defaulted() {
  U u;
  u.d.x = 4;
  return u.d.x;
}
static_assert(defaulted() == 4);
constexpr int nested() {
  N n{};
  n.inner.tr.x = 5;
  return n.inner.tr.x;
}
static_assert(nested() == 5);
constexpr int whole() {
  U u;
  u.tr = TR{6};
  return u.tr.x;
}
static_assert(whole() == 6);
// Nested unions along the chain: each member on it qualifies.
struct Mid {
  union {
    int j;
    TR tr;
  } in;
};
union W {
  int i;
  Mid m;
  constexpr W() : i(0) {}
};
constexpr int nested_unions() {
  W w;
  w.m.in.tr.x = 7;
  return w.m.in.tr.x;
}
static_assert(nested_unions() == 7);
