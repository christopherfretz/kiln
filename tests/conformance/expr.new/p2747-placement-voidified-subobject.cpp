// [expr.new]/24, [expr.const]/5.18 (C++26, P2747R2), [conv.ptr]/2: a user
// helper (not std::construct_at) that converts a pointer to a subobject
// to `void *` keeps designating that subobject, so a placement new of the
// subobject's type through the `void *` constructs it: a member at offset
// 0 (whose address is also the enclosing object's), a member at a nonzero
// offset, an array element, and a base class subobject, with the helper
// returning the `void *` through a call, a local and a cv-qualified
// `void *` as libc++'s __voidify does.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;

struct S { int first; int second; };
constexpr void *voidify(int &x) { return &x; }
constexpr void *voidify_cv(int &x) {
  return const_cast<void *>(static_cast<const volatile void *>(&x));
}
constexpr void *first_of(S &s) { return &s.first; }
constexpr void *second_of(S &s) { return &s.second; }
constexpr void *via_local(S &s) { void *p = &s.first; return p; }

constexpr int offset_zero() {
  S s{1, 2};
  int *p = new (first_of(s)) int(5);
  return s.first * 10 + s.second + (p == &s.first);
}
static_assert(offset_zero() == 53);

constexpr int offset_zero_ref() { S s{1, 2}; new (voidify(s.first)) int(6); return s.first; }
static_assert(offset_zero_ref() == 6);

constexpr int offset_zero_cv() { S s{1, 2}; new (voidify_cv(s.first)) int(7); return s.first; }
static_assert(offset_zero_cv() == 7);

constexpr int through_local() { S s{1, 2}; new (via_local(s)) int(8); return s.first; }
static_assert(through_local() == 8);

constexpr int nonzero_offset() { S s{1, 2}; new (second_of(s)) int(5); return s.first + s.second; }
static_assert(nonzero_offset() == 6);

constexpr int element() {
  int a[3] = {1, 2, 3};
  new (voidify(a[0])) int(7);
  new (voidify(a[2])) int(9);
  return a[0] + a[1] + a[2];
}
static_assert(element() == 18);

struct Pair { S s[2]; };
constexpr int nested_element() {
  Pair p{{{1, 2}, {3, 4}}};
  new (first_of(p.s[1])) int(30);
  return p.s[1].first + p.s[0].first;
}
static_assert(nested_element() == 31);

struct B { int v; };
struct D : B { int w; };
constexpr void *base_of(D &d) { return static_cast<B *>(&d); }
constexpr int base() {
  D d{};
  d.v = 1;
  d.w = 2;
  new (base_of(d)) B{7};
  return d.v + d.w;
}
static_assert(base() == 9);
