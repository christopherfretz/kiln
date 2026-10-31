// [expr.prim.lambda.capture]/12, [expr.const]: a reference capture of a
// variable whose lifetime has ended is not usable in a constant
// expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:59 note "read of variable whose lifetime has ended"
//@ diag: 12:30 note "in call to 'make().operator()()'"
//@ diag: 13:15 note "in call to 'use()'"
//@ diag: 11:29 note "declared here"
constexpr auto make() { int k = 3; auto l = [&k] { return k; }; return l; }
constexpr int use() { return make()(); }
static_assert(use() == 3);
