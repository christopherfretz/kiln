// [expr.const]/5, P2448R2: from C++23 clang 22 drops `CheckLiteralType` for a
// local variable of non-literal type, so a class with a virtual base fails at
// its (implicitly non-constexpr, before C++26) constructor instead.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
// Clang 22 twin of the virtual-base cases of non-literal-local.cpp; the other
// cases are non-literal-local-clang22.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:23 note "non-constexpr constructor 'M' cannot be used in a constant expression"
//@ diag: 22:15 note "in call to 'f()'"
//@ diag: 20:8 note "declared here"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:28 note "non-constexpr constructor 'M' cannot be used in a constant expression"
//@ diag: 24:15 note "in call to 'h()'"
//@ diag: 20:8 note "declared here"
struct V { constexpr V(int v = 0) : v(v) {} int v; };
struct M : virtual V { using V::V; };
constexpr int f() { M x[2]; return x[0].v; }
static_assert(f() == 0);
constexpr int h() { M x[2]{}; return 0; }
static_assert(h() == 0);
