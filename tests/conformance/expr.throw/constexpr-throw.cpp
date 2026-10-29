// [expr.const]/10: evaluating a throw-expression is not a core constant
// expression (clang 19 predates P3068); an unevaluated arm is fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "constexpr variable 'bad' must be initialized by a constant expression"
//@ diag: 8:45 note "subexpression not valid in a constant expression"
//@ diag: 10:21 note "in call to 'g(-1)'"
constexpr int g(int x) { return x > 0 ? x : throw 0; }
static_assert(g(3) == 3);
constexpr int bad = g(-1);
