// [dcl.constexpr]/1: a constexpr variable declaration shall be a definition.
// Found by the clang negative-test run (WP-912).
//@ std: c++23
//@ strict-diags
//@ expect: compile-fail
//@ diag: 7:22 error "constexpr variable declaration must be a definition"
extern constexpr int m;
