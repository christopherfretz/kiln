//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16.1 implements P2686R5 constexpr references to automatic objects
//@ diag: 31:20 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 31:20 note "pointer to 's' is not a constant expression"
//@ diag: 30:40 note "address of non-static constexpr variable 's' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 34:28 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 34:28 note "pointer to 's' is not a constant expression"
//@ diag: 33:15 note "address of non-static constexpr variable 's' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 35:29 error "constexpr variable 'sp' must be initialized by a constant expression"
//@ diag: 35:29 note "pointer to 's' is not a constant expression"
//@ diag: 33:15 note "address of non-static constexpr variable 's' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 36:22 error "variable 's' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 33:15 note "'s' declared here"
//@ diag: 36:3 note "lambda expression begins here"
//@ diag: 36:4 note "capture 's' by value"
//@ diag: 36:4 note "capture 's' by reference"
//@ diag: 36:4 note "default capture by value"
//@ diag: 36:4 note "default capture by reference"
// [expr.const.init]/2-4,9 (C++26): a constexpr local whose value points to
// itself is constexpr-referenceable only from its own function parameter
// scope: its address escaping the function (a namespace-scope constexpr
// variable initialized from it), a constexpr variable in a lambda body, or a
// static constexpr local ([expr.const.init]/4.3: representable at namespace
// scope) may not hold it; and in a lambda body the local itself is not
// constexpr-representable, so it is not usable in constant expressions
// there: naming it is an odr-use that needs a capture. Reduced from a find
// running GCC's C++ testsuite (WP-1680).
struct S { const S *self = this; int v = 1; };
constexpr const S *esc() { constexpr S s{}; return s.self; }
constexpr const S *p = esc();
void f() {
  constexpr S s{};
  [&] { constexpr const S *q = s.self; }();
  static constexpr const S *sp = s.self;
  [] { static_assert(s.self->v == 1); }();
}
