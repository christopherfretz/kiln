// [expr.const]/5, [basic.types.general]/10: a local variable of non-literal
// type cannot be created during constant evaluation. clang `CheckLiteralType`
// (from `EvaluateInPlace`) names the initializer's whole type, an array type
// included, at the initializer.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:23 note "non-literal type 'M[2]' cannot be used in a constant expression"
//@ diag: 29:15 note "in call to 'f()'"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:24 note "non-literal type 'NL[2][3]' cannot be used in a constant expression"
//@ diag: 32:15 note "in call to 'g()'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:27 note "non-literal type 'M[2]' cannot be used in a constant expression"
//@ diag: 34:15 note "in call to 'h()'"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:23 note "non-literal type 'Y' cannot be used in a constant expression"
//@ diag: 37:15 note "in call to 'k()'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:26 note "non-literal type 'NLD' cannot be used in a constant expression"
//@ diag: 42:15 note "in call to 'm()'"
//@ requires: clang<22
// clang 22 differs (no literal type check in C++23, P2448): see non-literal-local-22.cpp
struct V { constexpr V(int v = 0) : v(v) {} int v; };
struct M : virtual V { using V::V; };
constexpr int f() { M x[2]; return x[0].v; }
static_assert(f() == 0);
struct NL { NL(); int v; };
constexpr int g() { NL y[2][3]; return 0; }
static_assert(g() == 0);
constexpr int h() { M x[2]{}; return 0; }
static_assert(h() == 0);
struct Y { constexpr Y() {} ~Y(); };
constexpr int k() { Y y; return 0; }
static_assert(k() == 0);
constexpr int l() { auto f = [y = 1] { return y; }; return f(); }
static_assert(l() == 1);
struct NLD { int a; ~NLD(); };
constexpr int m() { NLD z{1}; return z.a; }
static_assert(m() == 1);
