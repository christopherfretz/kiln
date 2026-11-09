// [dcl.constexpr]/6: clang 19 does not constant-evaluate `__builtin_abs`
// (clang 20 added its `Constexpr` marker; the clang 22 shape is
// potential-constant-builtin-abs-clang22.cpp), so a call of it on the known
// path makes the function never constant (WP-945). spec/sema.md "Never
// constant".
//@ requires: clang<22
//@ std: c++20
//@ strict-diags
//@ expect: compile-fail
//@ diag: 12:15 error "constexpr function never produces a constant expression"
//@ diag: 12:34 note "subexpression not valid in a constant expression"
constexpr int a1(int x) { return __builtin_abs(-1) + x; }
