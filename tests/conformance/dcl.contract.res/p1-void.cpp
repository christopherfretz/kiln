// [dcl.contract.res]/1 (P2900R14): a postcondition of a function whose
// return type is void cannot have a result-name-introducer. GCC 16.1
// rejects it too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 11:15 error "function does not return a value to test"
//@ diag: 12:26 error "function does not return a value to test"
//@ diag: 13:30 error "function does not return a value to test"
void v() post(r: true);
struct S { void m() post(r: r); };
auto lam = []() -> void post(r: true) {};
