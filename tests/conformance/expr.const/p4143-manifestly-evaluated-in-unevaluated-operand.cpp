// [expr.const]/27 footnote as amended by P4143R0 (Croydon DR, "Constant
// evaluation fixes"): a manifestly constant-evaluated expression (here a
// template argument) is evaluated even within an unevaluated operand
// (decltype, sizeof, noexcept), while a call to an immediate function in
// an unevaluated operand is not an immediate invocation and is not
// evaluated. (The static_assert-message exception is pinned by
// tests/conformance/dcl.pre/static-assert-message-unevaluated-reflection.cpp.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:22 error "non-type template argument is not a constant expression"
//@ diag: 22:36 note "division by zero"
//@ diag: 28:22 note "in call to 'f(0)'"
//@ diag: 29:29 error "non-type template argument is not a constant expression"
//@ diag: 22:36 note "division by zero"
//@ diag: 29:29 note "in call to 'f(0)'"
//@ diag: 30:31 error "non-type template argument is not a constant expression"
//@ diag: 23:37 note "division by zero"
//@ diag: 30:31 note "in call to 'cf(0)'"
//@ diag: 31:29 error "non-type template argument is not a constant expression"
//@ diag: 22:36 note "division by zero"
//@ diag: 31:29 note "in call to 'f(0)'"
constexpr int f(int x) { return 10 / x; }
consteval int cf(int x) { return 10 / x; }
template <int N> struct X { static constexpr int v = N; };
template <class T> constexpr bool always = true;
using A = decltype(cf(0));                   // OK: not an immediate invocation
static_assert(sizeof(cf(0)) == sizeof(int)); // OK: not evaluated
using B = decltype(X<f(0)>{});               // error: template argument evaluated
constexpr auto s = sizeof(X<f(0)>);          // error
constexpr bool n = noexcept(X<cf(0)>::v);    // error
using C = decltype(always<X<f(0)>>);         // error
