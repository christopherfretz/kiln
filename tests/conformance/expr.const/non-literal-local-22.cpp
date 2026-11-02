// [expr.const]/5, [basic.types.general]/10: a local variable of non-literal
// type cannot be created during constant evaluation. clang `CheckLiteralType`
// (from `EvaluateInPlace`) names the initializer's whole type, an array type
// included, at the initializer.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
// Clang 22 twin of non-literal-local.cpp (P2448: no literal type check in C++23).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:23 note "non-constexpr constructor 'M' cannot be used in a constant expression"
//@ diag: 34:15 note "in call to 'f()'"
//@ diag: 32:8 note "declared here"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:24 note "non-constexpr constructor 'NL' cannot be used in a constant expression"
//@ diag: 37:15 note "in call to 'g()'"
//@ diag: 35:13 note "declared here"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:28 note "non-constexpr constructor 'M' cannot be used in a constant expression"
//@ diag: 39:15 note "in call to 'h()'"
//@ diag: 32:8 note "declared here"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:23 note "non-constexpr function '~Y' cannot be used in a constant expression"
//@ diag: 42:15 note "in call to 'k()'"
//@ diag: 40:29 note "declared here"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:25 note "non-constexpr function '~NLD' cannot be used in a constant expression"
//@ diag: 47:15 note "in call to 'm()'"
//@ diag: 45:21 note "declared here"
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
