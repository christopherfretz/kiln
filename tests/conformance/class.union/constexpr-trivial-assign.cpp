// [class.union.general], [class.copy.assign]/12: a union's trivial copy
// assignment copies its object representation, so the source's active
// member becomes the destination's (clang's handleTrivialCopy); libc++'s
// basic_string copy constructor assigns its __rep union this way.
// spec/oven.md "Unions".
//@ expect: compile-ok
union R {
  struct { char a; char sz; } s;
  long l;
};
constexpr int copy_struct_member() {
  R a{};
  a.s.sz = 3;
  R b;
  b = a;
  return b.s.sz;
}
static_assert(copy_struct_member() == 3);

union R2 { char c; long l; };
constexpr int copy_scalar_member() {
  R2 a{};
  a.l = 5;
  R2 b{};
  b = a;
  return b.l;
}
static_assert(copy_scalar_member() == 5);

struct Holder {
  R r;
  constexpr Holder() : r() { r.s.sz = 4; }
  constexpr Holder(const Holder &o) { r = o.r; }
};
constexpr int copy_in_constructor() {
  Holder h;
  Holder c(h);
  return c.r.s.sz;
}
static_assert(copy_in_constructor() == 4);
