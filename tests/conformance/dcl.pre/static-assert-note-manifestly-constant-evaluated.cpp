// [expr.const]/22, [dcl.pre]/12: the "expression evaluates to" note of a
// failed static_assert evaluates each operand of the failed comparison
// manifestly constant-evaluated, as the condition itself was: `if
// consteval` takes its compound statement and std::is_constant_evaluated()
// is true (clang 19 and 22 alike). Reduced from real code (WP-1943).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:15 error "static assertion failed due to requirement 'f() == 5'"
//@ diag: 41:19 note "expression evaluates to '1001 == 5'"
//@ diag: 42:15 error "static assertion failed due to requirement 'g() == 5'"
//@ diag: 42:19 note "expression evaluates to '2002 == 5'"
//@ diag: 43:15 error "static assertion failed due to requirement '5 > h()'"
//@ diag: 43:17 note "expression evaluates to '5 > 3003'"
//@ diag: 47:22 error "static assertion failed due to requirement 'f() == 1001'"
//@ diag: 47:26 note "expression evaluates to '1001 == 1001'"
//@ diag: 60:15 error "static assertion failed due to requirement 'bump() == f()'"
//@ diag: 60:22 note "expression evaluates to '12 == 1001'"
//@ diag: 65:15 error "static assertion failed due to requirement 'lam() == 0'"
//@ diag: 65:21 note "expression evaluates to '1002 == 0'"
//@ diag: 69:17 error "static assertion failed due to requirement 'f() + 7 == 0'"
//@ diag: 72:15 note "in instantiation of function template specialization 'k<7>' requested here"
//@ diag: 69:25 note "expression evaluates to '1008 == 0'"
#include <type_traits>

constexpr int f() {
  if consteval {
    return 1001;
  } else {
    return 1;
  }
}
constexpr int g() { return std::is_constant_evaluated() ? 2002 : 2; }
constexpr int h() { return __builtin_is_constant_evaluated() ? 3003 : 3; }
constexpr bool t() {
  if !consteval {
    return true;
  }
  return false;
}

static_assert(f() == 5);
static_assert(g() == 5);
static_assert(5 > h());
// The failing conjunct is found by a plain fold (t() is true there), as
// clang's findFailedBooleanCondition finds it; its note's operands are
// manifestly constant-evaluated.
static_assert(t() && f() == 1001);

// An operand that modifies a copy local to the evaluation.
struct C {
  int v;
  constexpr int inc() { return ++v; }
};
constexpr C gc{10};
constexpr int bump() {
  C c = gc;
  c.inc();
  return c.inc() + (std::is_constant_evaluated() ? 0 : 100);
}
static_assert(bump() == f());
constexpr auto lam = [] {
  C c{f()};
  return c.inc();
};
static_assert(lam() == 0);

// A static_assert in a function template, checked at instantiation.
template <int N> constexpr int k() {
  static_assert(f() + N == 0);
  return N;
}
static_assert(k<7>() == 7);
