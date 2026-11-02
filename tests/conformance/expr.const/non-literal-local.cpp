// [expr.const]/5, [basic.types.general]/10: a local variable of non-literal
// type cannot be created during constant evaluation. clang `CheckLiteralType`
// (from `EvaluateInPlace`) names the initializer's whole type, an array type
// included, at the initializer.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:23 note "non-literal type 'M[2]' cannot be used in a constant expression"
//@ diag: 27:15 note "in call to 'f()'"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:24 note "non-literal type 'NL[2][3]' cannot be used in a constant expression"
//@ diag: 30:15 note "in call to 'g()'"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:27 note "non-literal type 'M[2]' cannot be used in a constant expression"
//@ diag: 32:15 note "in call to 'h()'"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:23 note "non-literal type 'Y' cannot be used in a constant expression"
//@ diag: 35:15 note "in call to 'k()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:26 note "non-literal type 'NLD' cannot be used in a constant expression"
//@ diag: 40:15 note "in call to 'm()'"
//@ requires: clang<22
// WP-388 (vbase construction) for M at 22; the other cases are non-literal-local-clang22.cpp.
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
