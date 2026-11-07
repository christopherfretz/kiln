// [dcl.contract.res]/2, [expr.prim.id.unqual]/7 (P2900R14): the result
// name is a const lvalue, so a postcondition cannot modify the result
// object through it. GCC 16.1 rejects it too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 10:17 error "cannot modify 'r' in a contract predicate: it is implicitly const"
//@ diag: 11:21 error "cannot modify 'r' in a contract predicate: it is implicitly const"
int e() post(r: ++r) { return 1; }
int e2() post(r: (r = 2) != 0) { return 1; }
