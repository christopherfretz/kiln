// [dcl.constexpr]/6 since P2448 (C++23): a constexpr function that is never
// constant is valid; clang 19 checks it only under -Winvalid-constexpr
// (off by default, an error once on), a builtin it does not evaluate
// included (WP-945; without the flag nothing is reported).
// spec/sema.md "Never constant".
//@ std: c++23
//@ strict-diags
//@ expect: compile-fail
//@ args: -Winvalid-constexpr
//@ diag: 15:18 error "constexpr function never produces a constant expression"
//@ diag: 15:44 note "subexpression not valid in a constant expression"
//@ diag: 17:18 error "constexpr function never produces a constant expression"
//@ diag: 17:40 note "non-constexpr function 'fabs' cannot be used in a constant expression"
//@ diag: 16:19 note "declared here"
constexpr double w1(double x) { return x + __builtin_floor(1.0); }
extern "C" double fabs(double);
constexpr double w2(double x) { return fabs(2.0) + x; }
constexpr double w3(double x) { return fabs(x); }
