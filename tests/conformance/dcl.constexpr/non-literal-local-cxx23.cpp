// [dcl.constexpr], [expr.const]/10 (C++23, P2448R2): clang 22 no longer
// checks that a local variable of a constexpr function has a literal type;
// a volatile member is fine, and a class with a virtual base fails at its
// non-constexpr constructor. clang 19 still reports the non-literal type
// (non-literal-local-cxx23-19.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:24 note "non-constexpr constructor 'A' cannot be used in a constant expression"
//@ diag: 20:15 note "in call to 'vb()'"
//@ diag: 16:24 note "declared here"
struct V {};
struct Vol { volatile int m = 0; constexpr Vol() = default; };
struct A : virtual V { A() {} };
constexpr int vol() { Vol v; return 0; }
static_assert(vol() == 0);
constexpr int vb() { A x; return 0; }
static_assert(vb() == 0);
