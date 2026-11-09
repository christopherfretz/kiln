// [dcl.align]/3: alignas(type-id) requires a complete object type; 'void' is
// incomplete. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, alignas of an incomplete type-id is accepted
//@ expect: compile-fail
//@ diag: 7:15 error "invalid application of 'alignas' to an incomplete type 'void'"
class alignas(void) C {};
