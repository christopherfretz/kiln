// [meta.const.eval]/3 (P2641R4), [basic.life]/1, [class.base.init]/13,
// [class.cdtor]: during a constructor, a subobject whose initialization has
// not started (a later base or member, the member whose own initializer is
// being evaluated) is not within its lifetime; earlier subobjects, and
// members a mem-initializer list skips (default-initialized), are. A member
// of class type under construction is within its lifetime while its own
// constructor runs. A delegating constructor's body sees a constructed
// object. clang 20 added the builtin; clang 22 is the oracle.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1 has no __builtin_is_within_lifetime
#define L __builtin_is_within_lifetime

consteval int scalars(int w) {
  int x;
  if (w == 0) return L(&x);
  struct S { int a; int b; } s;
  if (w == 1) return L(&s.b);
  int* p = new int;
  bool r = L(p);
  delete p;
  return r;
}
static_assert(scalars(0) == 1 && scalars(1) == 1 && scalars(2) == 1);

struct Y {
  int a, b, c;
  consteval Y() : a(L(&b)), b(L(&a)), c(L(&c)) {}
};
static_assert(Y().a == 0 && Y().b == 1 && Y().c == 0);

struct B { int v; };
struct D : B {
  int w;
  consteval D() : B{L(&w) ? 5 : 6}, w(L(&v)) {}
};
static_assert(D().v == 6 && D().w == 1);

struct M {
  int x, y;
  consteval M(const int* later, bool& r) : x(1), y(0) { r = L(&x) && !L(later); }
};
struct E {};
struct W : E {
  int k;
  bool r = false;
  M m;
  int z;
  consteval W() : k(L(&k)), m(&z, r), z(L(&m.y) + L(&r) * 2) {}
};
static_assert(W().k == 0 && W().r && W().z == 3);

struct A {
  int a[2];
  int n;
  consteval A() : a{1, 2}, n(L(&a[1]) * 10 + L(&n)) {}
};
static_assert(A().n == 10);

struct Skip {
  int s;
  int t;
  consteval Skip() : t(L(&s)) {}
};
static_assert(Skip().t == 1);

struct Del {
  int d, e;
  consteval Del(int) : d(1), e(L(&d)) {}
  consteval Del() : Del(0) { e += L(&e) * 10; }
};
static_assert(Del().e == 11);

struct Agg { int a, b; };
struct H {
  Agg g;
  int t;
  consteval H() : g{L(&t), 2}, t(L(&g.b)) {}
};
static_assert(H().g.a == 0 && H().t == 1);
