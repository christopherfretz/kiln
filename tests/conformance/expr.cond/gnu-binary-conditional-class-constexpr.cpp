// [expr.cond], [expr.const] GNU `x ?: y` of class type in a constant
// expression: the Oven evaluates `x` once (one construction, then the
// copy into the result when `x` is chosen), converts it with its
// `operator bool`, and an lvalue result refers to `x` itself. WP-1738
// (GAP-052).
//@ expect: compile-ok
struct C {
  int v;
  int *count;
  constexpr C(int v, int *c) : v(v), count(c) { ++*count; }
  constexpr C(const C &o) : v(o.v), count(o.count) { ++*count; }
  constexpr explicit operator bool() const { return v != 0; }
};
constexpr int once(int start) {
  int n = 0;
  C r = C(start, &n) ?: C(42, &n);
  return r.v * 100 + n;
}
static_assert(once(5) == 502);
static_assert(once(0) == 4202);
constexpr int lvalue() {
  int n = 0;
  C a(1, &n), b(0, &n);
  C &r = a ?: b;
  r.v = 9;
  C &s = b ?: a;
  return a.v * 10 + (&s == &a) + n * 100;
}
static_assert(lvalue() == 291);
struct B {
  bool b;
  constexpr operator bool() const { return b; }
};
constexpr B pick(B x, B y) { return x ?: y; }
static_assert(pick(B{true}, B{false}).b);
static_assert(!pick(B{false}, B{false}).b);
struct I {
  int v;
  constexpr I(int v) : v(v) {}
};
constexpr int mixed(int k) {
  I c = k ?: I(3);
  return c.v;
}
static_assert(mixed(0) == 3);
static_assert(mixed(5) == 5);
