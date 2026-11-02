// [dcl.constexpr], [expr.const]/10 (C++23): clang 19 still applies
// CheckLiteralType to a local variable of a constexpr function during
// constant evaluation; clang 22 does not (non-literal-local-cxx23.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:27 note "non-literal type 'Vol' cannot be used in a constant expression"
//@ diag: 18:15 note "in call to 'vol()'"
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:24 note "non-literal type 'A' cannot be used in a constant expression"
//@ diag: 20:15 note "in call to 'vb()'"
struct V {};
struct Vol { volatile int m = 0; constexpr Vol() = default; };
struct A : virtual V { A() {} };
constexpr int vol() { Vol v; return 0; }
static_assert(vol() == 0);
constexpr int vb() { A x; return 0; }
static_assert(vb() == 0);
