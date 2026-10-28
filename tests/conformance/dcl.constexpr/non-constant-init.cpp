// [dcl.constexpr]/10: a constexpr variable needs a constant initializer
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 8:19 note "read of non-const variable 'v' is not allowed in a constant expression"
//@ diag: 7:5 note "declared here"
int v = 1;
constexpr int c = v;
