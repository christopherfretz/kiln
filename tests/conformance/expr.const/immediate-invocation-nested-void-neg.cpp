// [expr.const]/16-18 (P2564R3): in a constexpr function that is not a
// template specialization, g is not immediate-escalating, so the void
// call f(x) is an immediate invocation of its own; it modifies x, which
// is not created within that evaluation, so it is not a constant
// expression (rejected by GCC 16.1 too). In the escalated template
// g2<N> the nested void call is evaluated in place (spec/oven.md "Nested
// immediate invocations"; WP-537), so g2<0> is fine and g2<1>'s division
// by zero is diagnosed through the enclosing evaluation. Matches clang 19
// and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:32 error "call to consteval function 'f' is not a constant expression"
//@ diag: 19:28 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 20:32 note "in call to 'f(x)'"
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:45 note "division by zero"
//@ diag: 23:50 note "in call to 'h(x)'"
//@ diag: 25:15 note "in call to 'g2<1>()'"
consteval void f(int& x) { ++x; }
constexpr int g() { int x = 0; f(x); return x; }

consteval void h(int& x) { if (x > 0) x = 1 / (x - x); }
template <int N> constexpr int g2() { int x = N; h(x); return x; }
static_assert(g2<0>() == 0);
static_assert(g2<1>() == 0);
