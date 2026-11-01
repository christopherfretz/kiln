// [expr.const], [dcl.init.general]/16.6.2.2 (P0960): the oven evaluates
// parenthesized aggregate initialization of classes, unions (the first
// member becomes active), arrays (value-initialized tail, class elements
// default-constructed) and array new. Diagnostics match clang 19, which
// crashes on `newed` (`new int[n](4, 5)`, runtime bound; GCC 14 accepts).
//@ expect: compile-fail
//@ skip(clang): clang 19 crashes evaluating the runtime-bound array new in `newed`
//@ strict-diags
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:46 note "read of member 'f' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 34:15 note "in call to 'bad_union()'"
struct A { int x; int y; };
struct B { int x; long y; double z = 2.5; };
struct C : A { int z; };
union U { int i; float f; };
struct S { constexpr S() : v(7) {} constexpr S(int x) : v(x) {} int v; };
constexpr int k() { A a(3, 4); int arr[3](1, 2); B b(1); return a.x + a.y + arr[1] + arr[2] + (int)b.z; }
static_assert(k() == 11);
constexpr int base() { C c(A(1, 2), 3); return c.x * 100 + c.y * 10 + c.z; }
static_assert(base() == 123);
constexpr int uni() { U u(5); return u.i; }
static_assert(uni() == 5);
constexpr int cls() { S s[3](S(1)); return s[0].v * 100 + s[1].v * 10 + s[2].v; }
static_assert(cls() == 177);
constexpr int newed(int n) {
  int* p = new int[n](4, 5);
  int r = p[0] * 100 + p[1] * 10 + p[n - 1];
  delete[] p;
  return r;
}
static_assert(newed(3) == 450);
constexpr float bad_union() { U u(1); return u.f; }
constexpr A ga(1, 2);
static_assert(bad_union() == 0);
static_assert(ga.x == 1 && ga.y == 2);
