// [expr.prim.lambda.capture]/11, [basic.def.odr]/5, [expr.const]/10: a
// constant expression evaluated in a lambda body that passes a by-copy
// capture to a call (the object of a member function call, an argument
// bound to a reference or a pointer parameter, a copy) odr-uses it, so by
// the text the name denotes the closure's member, which needs the closure
// object: no constant expression. clang 22 and GCC 16.1 both evaluate these
// forms against the captured variable's value, and kiln follows them (a
// recorded deviation, docs/status/deviations.md, WP-1655). The capture is
// unchanged: the variable is still captured (the closure's size below), and
// at run time the body reads the closure's member (the mutable lambda
// changes its copy's mutable member and reads it back). Verdicts of the
// forms (all three accept): member call reading data, returning a
// reference to a member, through `(*&p)`, `(&p)->`, a cast to a reference,
// a conditional, `.*`; conversion function; member and non-member
// operators; `get(p)`, `getp(&p)`, a function template; a copy (`= p`,
// `{p}`, `P{p}`, `P(p)`, a non-trivial constexpr copy constructor);
// `id(n)`, `id(*&n)`, `id(a[1])`, `id(p.v)`, `first(a)`, `first(p.a)`;
// `[p]`, `[=]`, a `mutable` lambda, a `static_assert`, `if constexpr`, a
// template argument; a nested lambda, a lambda inside a generic lambda, a
// lambda in a function template (also with a dependent type); a nested
// `[&]` lambda called in the constant expression; a constant expression in
// a `[&]` (or generic `[&]`) lambda inside the `[=]` one, which names the
// outer copy and is read in place (`*&n`); a structured binding
// (constexpr-call-reads-by-copy-capture-binding-run.cpp). The forms kept
// rejected are in constexpr-call-reads-by-copy-capture-rejected.cpp. Found
// running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
//@ args: -std=c++26 -Wno-unused-lambda-capture
struct P {
  int v;
  int a[2];
  constexpr int get() const { return v; }
  constexpr const int& ref() const { return v; }
  constexpr operator int() const { return v; }
  constexpr int operator+() const { return v; }
  constexpr int operator[](int i) const { return a[i]; }
};
struct Q {
  int v;
  constexpr Q(int x) : v(x) {}
  constexpr Q(const Q& o) : v(o.v) {}
};
struct M {
  mutable int c;
  int v;
  constexpr int get() const { return v; }
};
struct W { P p; };
constexpr int operator-(const P& p) { return -p.v; }
constexpr int get(const P& p) { return p.v; }
constexpr int getp(const P* p) { return p->v; }
constexpr int id(const int& x) { return x; }
constexpr int first(const int* q) { return *q; }
template <class T> constexpr int tget(const T& t) { return t.get(); }
template <int N> struct X { static constexpr int value = N; };

int forms() {
  constexpr P p{3, {4, 5}};
  const int n = 6;
  constexpr int a[2] = {7, 8};
  auto l1 = [=] {
    constexpr int j1 = p.get();
    constexpr int j2 = p.ref();
    constexpr int j3 = (*&p).get();
    constexpr int j4 = (&p)->get();
    constexpr int j5 = static_cast<const P&>(p).get();
    constexpr int j6 = (true ? p : p).get();
    constexpr auto pm = &P::get;
    constexpr int j7 = (p.*pm)();
    constexpr int j8 = p;
    constexpr int j9 = +p + -p + p[1];
    constexpr int j10 = get(p) + getp(&p) + tget(p);
    constexpr P c1 = p;
    constexpr P c2{p};
    constexpr W w{p};
    constexpr int j11 = P{p}.v + P(p).v + c1.v + c2.v + w.p.v;
    constexpr int j12 = id(n) + id(*&n) + id(a[1]) + id(p.v) + first(a) + first(p.a);
    constexpr int j13 = [&] { return p.get() + *&n; }();
    static_assert(p.get() == 3);
    if constexpr (p.get() != 3)
      return 0;
    return j1 + j2 + j3 + j4 + j5 + j6 + j7 + j8 + j9 + j10 + j11 + j12 + j13 +
           X<p.get()>::value;
  };
  auto l2 = [p] { constexpr int j = p.get(); return j; };
  auto l3 = [=] { return [=] { constexpr int j = p.get(); return j; }(); };
  auto l4 = [=](auto x) { return [=] { constexpr int j = p.get(); return j + x; }(); };
  constexpr Q q{9};
  auto l5 = [=] { constexpr Q c = q; constexpr Q d(q); return c.v + d.v; };
  // A capture of a capture: the inner `[&]` names the outer closure's copy
  // ([expr.prim.lambda.capture]/12); read in place, also in a generic one.
  auto l6 = [=] {
    return [&] { constexpr int j = *&n + (*&p).v; static_assert(*&n == 6); return j; }() +
           [&](auto) { constexpr int j = *&n; return j; }(0) +
           [&](int k) { switch (k) { case *&n: return 0; } return 1; }(6);
  };
  static_assert(sizeof(l2) == sizeof(P));
  static_assert(sizeof(l3) == sizeof(P));
  return l1() + l2() + l3() + l4(0) + l5() + l6();
}

template <class T> int in_template() {
  constexpr T p{3, {4, 5}};
  constexpr P r{1, {0, 0}};
  auto l = [=] { constexpr int j = p.get() + r.get(); return j; };
  static_assert(sizeof(l) == 2 * sizeof(P));
  return l();
}

int mutable_copy() {
  constexpr M m{0, 3};
  auto l = [=]() mutable {
    m.c = 5;
    constexpr int j = m.get();
    return j * 100 + m.get() * 10 + m.c;
  };
  static_assert(sizeof(l) == sizeof(M));
  return l();
}

int main() {
  // 3+3+3+3+3+3+3+3 + (3-3+5) + 9 + 15 + (6+6+8+3+7+4) + 9 + 3 = 99
  if (forms() != 99 + 3 + 3 + 3 + 18 + 15)
    return 1;
  if (in_template<P>() != 4)
    return 2;
  if (mutable_copy() != 335)
    return 4;
  return 0;
}
