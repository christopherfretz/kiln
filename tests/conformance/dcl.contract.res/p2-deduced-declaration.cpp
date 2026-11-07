// [dcl.contract.res]/2 (P2900R14): a postcondition with a result name on a
// function with a deduced return type appears only on its definition (the
// result name's type is not known otherwise). GCC 16.1 rejects it too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 9:17 error "postcondition with a result name on a function with a deduced return type must appear on its definition"
auto ded() post(r: r > 0);
auto ok() post(r: r > 0) { return 1; }
decltype(auto) ok2() post(r: r > 0) { return 1; }
