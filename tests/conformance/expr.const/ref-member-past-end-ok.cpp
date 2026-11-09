// [dcl.ref]/5, [expr.add]/4: the valid twin of ref-member-past-end-clang22.cpp
// and ref-past-end-result.cpp. Reference members bound to the last element,
// a pointer member one past the end, and a reference to the complete array
// are constant. clang checks neither a reference parameter nor a lambda's
// init-capture as it binds, so kiln does not either. WP-1021.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
constexpr int a[2] = {1, 2};
constexpr const int *end = a + 2;
struct R { const int &r; };
struct D { const int &r = *(end - 1); };
struct C { const int &r; constexpr C(const int *p) : r(p[1]) {} };
struct B : R {};
struct E { const int *e; const int (&all)[2]; };
constexpr int f(const int *p) {
  R x{p[1]};
  R y{.r = p[0]};
  R *q = new R{p[1]};
  int s = x.r + y.r + q->r;
  delete q;
  D d;
  C c(p);
  B b{{p[1]}};
  return s + d.r + c.r + b.r + R{p[0]}.r;
}
static_assert(f(a) == 12);
constexpr R g{*(end - 1)};
static_assert(g.r == 2);
constexpr E e{end, a};
static_assert(e.e - e.all == 2);
constexpr int h(const int &) { return 3; }
constexpr int param(const int *p) { return h(p[2]); }
static_assert(param(a) == 3);
constexpr int capture(const int *p) {
  auto l = [&r = p[2]] { return 4; };
  return l();
}
static_assert(capture(a) == 4);
