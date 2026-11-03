// [expr.const]/10: clang 19 reports the non-literal type of a class with a
// virtual base (clang 22 twin: constexpr-vbase-construct-cxx23.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:23 note "non-literal type 'Y' cannot be used in a constant expression"
//@ diag: 23:15 note "in call to 'a()'"
//@ diag: 26:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:23 note "non-literal type 'Z' cannot be used in a constant expression"
//@ diag: 26:15 note "in call to 'b()'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:23 note "non-literal type 'P' cannot be used in a constant expression"
//@ diag: 29:15 note "in call to 'c()'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:23 note "non-literal type 'Y' cannot be used in a constant expression"
//@ diag: 31:15 note "in call to 'd()'"
struct V { int v = 1; };
struct W : virtual V { };
struct Y : W {};
constexpr int a() { Y y; return 0; }
static_assert(a() == 0);
struct Z : W { Z() = default; };
constexpr int b() { Z z; return 0; }
static_assert(b() == 0);
struct P : private W { };
constexpr int c() { P z; return 0; }
static_assert(c() == 0);
constexpr int d() { Y y{}; return 0; }
static_assert(d() == 0);
