// [dcl.init.ref], [expr.call]/7, [expr.sizeof]/2: references in constant
// evaluation. A reference variable, parameter or member holds its
// referent's address; reads and writes go through it.
constexpr void set(unsigned &x) { x = 7; }
constexpr void swap(unsigned &x, unsigned &y) { unsigned t = x; x = y; y = t; }
constexpr unsigned a1() { unsigned a = 1; set(a); return a; }
static_assert(a1() == 7);
constexpr unsigned a2() { unsigned a[2] = {1, 2}; swap(a[0], a[1]); return a[0] * 10 + a[1]; }
static_assert(a2() == 21);
constexpr unsigned a3() { unsigned a[2] = {1, 2}; unsigned &r = a[0]; r = 9; return a[0]; }
static_assert(a3() == 9);
constexpr void bump(unsigned &x) { set(x); x += 1; }
constexpr unsigned a4(unsigned v) { bump(v); return v; }
static_assert(a4(0) == 8);
struct S { int m; int n[2]; };
constexpr int a5() { S s{1, {2, 3}}; int &r = s.n[1]; r = 4; const int &c = s.m; return s.n[1] + c; }
static_assert(a5() == 5);
struct R { int &r; int v; };
constexpr int a6() { int z = 4; R x{z, 1}; x.r = 9; return z + x.v + x.r; }
static_assert(a6() == 19);
static_assert(sizeof(int &) == 4 && alignof(long &) == 8);
constexpr int z0 = 5;
constexpr const int &gr = z0;
static_assert(gr == 5 && &gr == &z0);
constexpr int arr[3] = {1, 2, 3};
constexpr const int &ge = arr[2];
static_assert(ge == 3);
constexpr int sum(const int (&a)[3]) { return a[0] + a[1] + a[2]; }
static_assert(sum(arr) == 6);
constexpr void isort(unsigned *a, int n) {
  for (int i = 1; i < n; ++i)
    for (int j = i; j > 0 && a[j - 1] > a[j]; --j) swap(a[j - 1], a[j]);
}
constexpr unsigned a7() { unsigned a[4] = {3, 1, 4, 2}; isort(a, 4); return a[0] * 1000 + a[1] * 100 + a[2] * 10 + a[3]; }
static_assert(a7() == 1234);
