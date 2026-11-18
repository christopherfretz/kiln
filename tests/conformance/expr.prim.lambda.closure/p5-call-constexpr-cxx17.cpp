// [expr.prim.lambda.closure]/5, /8 (P0170R1): from C++17 the call operator
// and the conversion function to pointer to function are constexpr and a
// lambda-expression may appear in a constant expression; the forms
// `p5-call-not-constexpr-before-cxx17.cpp` rejects at C++14 are accepted.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ args: -Wno-unused-value

constexpr int a = []{ return 1; }();
int f() { constexpr int b = []{ return 1; }(); return b; }
auto l = []{ return 1; };
constexpr int c = l();
constexpr int (*p1)() = []{ return 1; };
constexpr int (*p2)() = l;
constexpr int (*p3)() = +l;
static_assert([]{ return true; }(), "");
enum E { e = []{ return 1; }() };
int g() { switch (1) { case []{ return 1; }(): return 1; } return 0; }
template<class T> void m() { enum { e = []{ return 1; }() }; }
template void m<int>();
constexpr int d = ([]{}, 0);
static_assert(([]{ return true; }, true), "");
