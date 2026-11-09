// [expr.const]/2: RecurseB's initializer read RecurseA before RecurseA's
// initializer was seen, so RecurseB is not usable in constant expressions.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 12:15 error "constexpr variable 'RecurseC' must be initialized by a constant expression"
//@ diag: 12:26 note "initializer of 'RecurseB' is not a constant expression"
//@ diag: 10:11 note "declared here"
extern const int RecurseA;
const int RecurseB = RecurseA;
const int RecurseA = 10;
constexpr int RecurseC = RecurseB;
