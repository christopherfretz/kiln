// [expr.call]/7, [expr.const]/10: a parameter object is destroyed when the
// call ends; in constant evaluation clang names it by the callee's first
// declaration of the parameter ("in call to 'first.~D()'", '.~D()' for an
// unnamed one) and locates the destructor call there. [expr.call]/12: a
// literal class with a non-trivial copy constructor passed through the
// ellipsis is clang's `(__builtin_trap(), arg)`, an invalid subexpression.
// Same at -fclang-compat=19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 21 note "null pointer"
//@ diag: 24:18 note "in call to '.~D()'"
//@ diag: 29:15 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 21 note "null pointer"
//@ diag: 25:19 note "in call to 'first.~D()'"
//@ diag: 30:24 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 31:15 error "constexpr variable 'm' must be initialized by a constant expression"
//@ diag: 21 note "null pointer"
//@ diag: 23:30 note "in call to 'x.~D()'"
// (The null-pointer note is worded and placed differently by the releases.)
struct D { int *p = nullptr; constexpr D() {} constexpr D(const D &) {} constexpr ~D() { *p = 1; } };
struct T { constexpr T() {} constexpr T(const T &) {} constexpr ~T() {} };
struct S { constexpr int m(D x) const { return 1; } };
constexpr int h(D) { return 1; }
constexpr int g(D first);
constexpr int g(D second) { return 1; }
constexpr int v(int, ...) { return 1; }
constexpr int b = h(D());
constexpr int f = g(D());
constexpr int c = v(1, T());
constexpr int m = S().m(D());
