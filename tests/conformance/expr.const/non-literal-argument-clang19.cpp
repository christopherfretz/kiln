// [expr.const]/10, [basic.types.general]/10: a by-value argument of non-literal
// class type (to a declared or a variadic parameter) is a prvalue initializing
// a parameter object, not the object the evaluated declaration initializes:
// clang `CheckLiteralType` stops the evaluation there, with no note about the
// constructor or destructor it would have run.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 21:24 note "non-literal type 'NL' cannot be used in a constant expression"
//@ diag: 22:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 22:21 note "non-literal type 'NL' cannot be used in a constant expression"
//@ diag: 23:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 23:27 note "non-literal type 'NL' cannot be used in a constant expression"
//@ requires: clang<22
// clang 22 drops this check from C++23 (P2448R2): non-literal-argument.cpp.
struct NL { constexpr NL() {} constexpr NL(const NL &) {} ~NL() {} };
struct L { int x = 0; };
constexpr int v(int, ...) { return 1; }
constexpr int h(NL) { return 1; }
constexpr int lit = v(1, L());
constexpr int a = v(1, NL());
constexpr int b = h(NL());
constexpr int c = v(1, 2, (NL()));
