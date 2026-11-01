// [dcl.init.aggr]/5: a union initialized by an empty braced list
// initializes its variant member with a default member initializer (clang
// `CheckStructUnionTypes` picks it; a `CXXDefaultInitExpr`), also for an
// anonymous union member and in a functional cast, so the globals hold
// that member and local initialization stores it. Plus GNU array range
// designators (`[a ... b] =`), which initialize every element of the range.
struct M { int x = 4; union { int i; float f = 2.0f; }; };
union N { int i; float f = 3.0f; };
struct W { union { struct { int a; int b = 6; }; long l; }; };
M m2 = M{}; M m3 = {}; M m4;
N n1 = {}; N n2 = N{}; N n3;
W w1 = {};
int r1[6] = {[1 ... 3] = 7, [5] = 9};
float g() { M m{}; N n{}; W w{}; return m.f + n.f + float(w.b); }
int h(int k) {
  int a[5] = {[1 ... 3] = k, [4] = k + 1};
  int m[2][3] = {[1] = {[0 ... 2] = k}, [0][1] = 4};
  return a[0] + a[1] + a[3] + a[4] + m[1][2] + m[0][1];
}
