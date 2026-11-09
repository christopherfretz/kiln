// [dcl.constexpr]/1: a constexpr variable declaration shall be a definition.
// Found by the clang negative-test run (WP-912).
//@ std: c++23
//@ xfail(kiln): WP-912 follow-up, non-defining constexpr variable declarations are accepted
//@ expect: compile-fail
//@ diag: 7:22 error "constexpr variable declaration must be a definition"
extern constexpr int m;
