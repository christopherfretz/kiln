// __is_invocable needs every operand complete, void or an array of unknown
// bound (as GCC 16.1); a reference to an incomplete class is fine. Found
// running GCC's C++ testsuite. WP-1667.
//@ std: c++20
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __is_invocable
//@ strict-diags
//@ diag: 14:25 error "incomplete type 'Incomplete' used in type trait expression"
//@ diag: 13:8 note "forward declaration of 'Incomplete'"
//@ diag: 15:49 error "incomplete type 'Incomplete' used in type trait expression"
//@ diag: 13:8 note "forward declaration of 'Incomplete'"
struct Incomplete;
bool a = __is_invocable(Incomplete);
bool b = __is_invocable(void (*)(Incomplete &), Incomplete);
bool c = __is_nothrow_invocable(void (*)(Incomplete &), Incomplete &);
