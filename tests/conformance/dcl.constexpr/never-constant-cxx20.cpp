// [dcl.constexpr]/6 (until C++23): a constexpr function for which no
// argument values produce a constant expression is ill-formed (clang
// diagnoses it). Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, the pre-C++23 never-a-constant-expression check is missing
//@ expect: compile-fail
//@ diag: 8:15 error "constexpr function never produces a constant expression"
constexpr int f() { return throw 0, 0; }
