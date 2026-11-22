// [expr.const], [dcl.constexpr]: the values of calls the Oven inlines at
// their call sites (spec/oven.md "Inlining": short leaf bodies; a site
// is inlined at its first execution) and of calls it does not inline
// (memoized, not a leaf, recursive, virtual, a constructor, a body with
// a frame object) are the values of the calls, in constexpr and
// consteval callers alike. `if consteval` in an inlined body takes the
// branch of its caller's evaluation.
//@ expect: compile-ok
#include <array>
#include <string_view>
// Qualifying: short leaf bodies, called repeatedly from loops.
struct Span {
  const int *p;
  int n;
  constexpr int size() const { return n; }
  constexpr const int &operator[](int i) const { return p[i]; }
  constexpr bool empty() const { return n == 0; }
};
constexpr int abs_at(const int *p) { return *p < 0 ? -*p : *p; }
constexpr int clamp_at(const int *p, int lo, int hi) {
  if (*p < lo)
    return lo;
  if (*p > hi)
    return hi;
  return *p;
}
constexpr double half(const double *d) { return *d / 2; }
constexpr int which(const int *p) {
  if consteval {
    return *p + 1000;
  } else {
    return *p;
  }
}
constexpr void bump(int *p, int by) { *p += by; }
constexpr int leaves() {
  int a[6] = {3, -1, 4, -1, 5, -9};
  Span s{a, 6};
  int t = 0;
  for (int i = 0; !s.empty() && i < s.size(); ++i) {
    t += s[i];
    t += abs_at(&s[i]) * 10;
    t += clamp_at(&a[i], -1, 4) * 100;
    t += which(&a[i]);
  }
  int c = 0;
  for (int i = 0; i < 10; ++i)
    bump(&c, i);
  double d = 9;
  return t + c + int(half(&d) * 2);
}
static_assert(leaves() == 1 + 230 + 800 + 6001 + 45 + 9);
// The same callee at two call sites with different argument registers.
constexpr int two_sites(int n) {
  int x = n, y = -n;
  int s = 0;
  for (int i = 0; i < 4; ++i) {
    s += abs_at(&x);
    int k = i * 3;
    s += abs_at(&y) + k;
  }
  return s;
}
static_assert(two_sites(5) == 58);
static_assert(two_sites(7) == 74);
// Not qualifying: memoized (scalar parameters), not a leaf, recursive,
// virtual, a constructor, a body with a frame object, consteval.
constexpr int sq(int v) { return v * v; }
constexpr int sq_twice(const int *p) { return sq(*p) + sq(*p); }
constexpr int down(const int *p, int n) { return n == 0 ? *p : down(p, n - 1); }
struct Base {
  constexpr virtual int get() const { return 1; }
};
struct Derived : Base {
  constexpr int get() const override { return 2; }
};
struct Pt {
  int x;
  constexpr Pt(int v) : x(v) {}
};
constexpr int local_array(const int *p) {
  int b[2] = {*p, *p};
  return b[0] + b[1];
}
consteval int imm(const int *p) { return *p * 3; }
constexpr int others() {
  int v = 4;
  Derived d;
  const Base &b = d;
  int s = 0;
  for (int i = 0; i < 3; ++i) {
    s += sq(i) + sq_twice(&v) + down(&v, 3) + b.get() + Pt(i).x +
         local_array(&v);
  }
  return s;
}
static_assert(others() == 5 + 3 * (32 + 4 + 2 + 8) + 3);
consteval int imm_loop() {
  int v = 2, s = 0;
  for (int i = 0; i < 3; ++i)
    s += imm(&v);
  return s;
}
static_assert(imm_loop() == 18);
// Library leaves: string_view's size() and an array's operator[].
constexpr int lib() {
  std::array<int, 4> a{1, 2, 3, 4};
  std::string_view sv = "kiln";
  int s = 0;
  for (std::size_t i = 0; i < a.size(); ++i)
    s += a[i] * int(sv.size());
  return s;
}
static_assert(lib() == 40);
