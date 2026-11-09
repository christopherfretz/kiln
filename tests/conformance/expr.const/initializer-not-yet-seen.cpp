// [expr.const]/2: RecurseB's initializer read RecurseA before RecurseA's
// initializer was seen, so RecurseB is not usable in constant expressions.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, the evaluator reads an initializer that was not yet visible
//@ expect: compile-fail
//@ diag: 11:15 error "constexpr variable 'RecurseC' must be initialized by a constant expression"
extern const int RecurseA;
const int RecurseB = RecurseA;
const int RecurseA = 10;
constexpr int RecurseC = RecurseB;
