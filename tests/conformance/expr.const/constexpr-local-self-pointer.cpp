//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16.1 implements P2686R5 constexpr references to automatic objects
// [expr.const.init]/2-4 (C++26): a constexpr local whose value points into
// itself is constant-initializable. Immediately after its declaration every
// object its constituent pointers and references designate is an automatic
// object whose variable has the same function parameter scope, so it is
// constexpr-referenceable, and the value is constexpr-representable there:
// a pointer or reference member to the object itself, a pointer to its own
// member, array elements pointing to each other, a closure capturing a local
// by reference, a structured binding whose object points to itself, in a
// template, a generic lambda and an immediate function. Clang 22 and GCC
// 16.1 reject all of these (the C++23 rule, constexpr-local-self-pointer-cxx23.cpp).
// Reduced from a find running GCC's C++ testsuite (WP-1680).
struct S { const S *self = this; int v = 1; };
struct R { const R &self = *this; int v = 2; };
struct U { int v = 3; const int *p = &v; };
struct N { const N *next; int v; };
constexpr bool f(int parm) {
  constexpr S s{};
  static_assert(s.self == &s && s.self->v == 1);
  constexpr R r{};
  static_assert(&r.self == &r && r.self.v == 2);
  constexpr U u{};
  static_assert(u.p == &u.v && *u.p == 3);
  constexpr N a[2] = {{&a[1], 4}, {&a[0], 5}};
  static_assert(a[0].next->v == 5 && a[1].next->next == &a[1]);
  int k = parm;
  constexpr auto l = [&k] { return k; };
  constexpr const int *pk = &k;
  constexpr struct { const int *p; } pp{&parm};
  static_assert(pk == &k && pp.p == &parm);
  constexpr auto [x, y] = S{};
  static_assert(y == 1);
  return s.self == &s && l() == parm && x->v == 1;
}
static_assert(f(7));
template <class T> constexpr bool t() { constexpr T x{}; return x.self == &x; }
static_assert(t<S>());
consteval int c() { constexpr N a[2] = {{&a[1], 1}, {&a[0], 2}}; return a[0].next->v * 10 + a[1].next->v; }
static_assert(c() == 21);
constexpr bool rec(int n) {
  constexpr S s{};           // a fresh object per invocation, pointing to itself
  return n ? rec(n - 1) && s.self == &s : s.self == &s;
}
static_assert(rec(3));
constexpr int gl = [](auto z) { constexpr S s{}; return s.self == &s ? z : 0; }(4);
static_assert(gl == 4);
// Namespace-scope and static constexpr objects: well-formed at every -std.
constexpr S g{};
static_assert(g.self == &g);
constexpr bool h() { static constexpr S st{}; return st.self == &st; }
static_assert(h());
