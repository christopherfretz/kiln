// [dcl.pre]/[expr.const]: a static_assert whose constant condition is false
// is ill-formed; the note shows the evaluated operands.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion failed"
//@ diag: 9:20 note "evaluates to '4 == 5'"
constexpr int f(int x) { return x * 2; }

static_assert(f(2) == 5);
