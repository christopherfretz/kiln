// [expr.const]/5, P2448R2: from C++23 on a local variable of non-literal type
// may be created during constant evaluation; clang 22 drops `CheckLiteralType`
// there, so a non-constexpr constructor or destructor of the variable is what
// fails (noted at the declaration), and a lambda closure is fine.
// (non-literal-local.cpp: clang 19's non-literal-type notes, kept at
// -fclang-compat=19; its virtual-base cases at 22 are WP-388's.)
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:24 note "non-constexpr constructor 'NL' cannot be used in a constant expression"
//@ diag: 28:15 note "in call to 'g()'"
//@ diag: 26:13 note "declared here"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:23 note "non-constexpr function '~Y' cannot be used in a constant expression"
//@ diag: 31:15 note "in call to 'k()'"
//@ diag: 29:29 note "declared here"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:25 note "non-constexpr function '~NLD' cannot be used in a constant expression"
//@ diag: 36:15 note "in call to 'm()'"
//@ diag: 34:21 note "declared here"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:29 note "non-constexpr constructor 'NL' cannot be used in a constant expression"
//@ diag: 38:15 note "in call to 'n()'"
//@ diag: 26:13 note "declared here"
struct NL { NL(); int v; };
constexpr int g() { NL y[2][3]; return 0; }
static_assert(g() == 0);
struct Y { constexpr Y() {} ~Y(); };
constexpr int k() { Y y; return 0; }
static_assert(k() == 0);
constexpr int l() { auto f = [y = 1] { return y; }; return f(); }
static_assert(l() == 1);
struct NLD { int a; ~NLD(); };
constexpr int m() { NLD z{1}; return z.a; }
static_assert(m() == 1);
constexpr int n() { NL y[2]{}; return 0; }
static_assert(n() == 0);
