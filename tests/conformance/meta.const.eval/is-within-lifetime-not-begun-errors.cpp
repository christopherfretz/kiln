// [meta.const.eval]/4 (P2641R4), [basic.life]/1, [expr.const]: within the
// initializer of a variable being constant-initialized, a pointer into that
// variable designates an object whose lifetime has not yet begun, and
// `__builtin_is_within_lifetime` fails on it (clang's
// `err_invalid_is_within_lifetime`); a local variable or a temporary
// under construction answers false for its not-yet-initialized members.
// (The builtin is an immediate invocation using `this` in a constexpr
// constructor, which is diagnosed too.)
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:21 error "call to consteval function '__builtin_is_within_lifetime' is not a constant expression"
//@ diag: 19:51 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 21:13 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 19:21 note "'__builtin_is_within_lifetime' cannot be called with a pointer to an object whose lifetime has not yet begun"
//@ diag: 21:13 note "in call to 'H()'"
struct H {
  int a, b;
  constexpr H() : a(__builtin_is_within_lifetime(&b)), b(1) {}
};
constexpr H h;
constexpr int f() { H x; return x.a; }
static_assert(f() == 0);
constexpr int g = H().a;
static_assert(g == 0);
