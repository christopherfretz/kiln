// [basic.def.odr]/5, [expr.const]/4 (WP-231): a reference variable usable
// in constant expressions is not odr-used where it is named; codegen reads
// through its constant initializer (clang `EmitDeclRefLValue`'s
// `NOUR_Constant` path) instead of loading the reference, and folds the
// read of a reference to a constant object to the value (clang
// `tryEmitAsConstant`, `CEK_AsValueOrReference`).
int x = 1;
int& r1 = x;
const int& r2 = 5;
struct P { int a, b; };
const P& r3 = P{1, 2};
extern int y;
int& r4 = y;
int* p = &x;
int& r5 = *p;
constexpr int c = 3;
const int& r6 = c;
int arr[3] = {1, 2, 3};
int& r7 = arr[1];
const int& a = {}, b = 5;
void g();
void (&fr)() = g;
struct S { int x; mutable int m; };
const S& rs = S{1, 2};
volatile int vi = 3;
volatile int& rv = vi;
int&& rr = 7;
const double& rd = 1;
int f1() { return r1; }
int f2() { return r2; }
int f3() { return r3.b; }
int f4() { return r4; }
int f5() { return r5; }
int f6() { return r6; }
int* f7() { return &r1; }
int f8() { const int& lr = c; return lr; }
int f9() { int& lr = x; return lr; }
int f10() { return r7; }
int f11() { int z = 0; int& lr = z; return lr; }
int f12() { const int& lr = 5; return lr; }
int f13() { return a + b; }
void f14() { fr(); }
int f15() { return rs.m + rs.x; }
int f16() { return rv; }
int f17() { return rr; }
double f18() { return rd; }
int f19() { static int s = 1; int& lr = s; return [] { return lr; }(); }
template <class T> T f20() { static T t{}; T& r = t; return r; }
int i20 = f20<int>();
