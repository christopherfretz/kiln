// [dcl.contract.func]/1 (P2900R14): only a postcondition-specifier has a
// result-name-introducer; in a precondition `r:` is not a declaration, so
// the name is undeclared. GCC 16.1 rejects it too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 11:19 error "use of undeclared identifier 'r'"
//@ diag: 11:20 error "expected ')'"
//@ diag: 11:18 note "to match this '('"
int pr(int x) pre(r: x);
