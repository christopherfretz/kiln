// [dcl.constexpr]/10: a constexpr variable must be initialized
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:15 error "constexpr variable 'c' must be initialized by a constant expression"
constexpr int c;
