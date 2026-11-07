// [dcl.contract.func]/4 (P2900R14): a declaration that is not a first
// declaration may omit the function-contract-specifier-seq but cannot add
// one the first declaration lacks. GCC 16.1 rejects these too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 15:15 error "declaration adds function contract specifiers to 'h1'"
//@ diag: 14:5 note "previous declaration is here"
//@ diag: 17:15 error "declaration adds function contract specifiers to 'h2'"
//@ diag: 16:5 note "previous declaration is here"
//@ diag: 19:18 error "declaration adds function contract specifiers to 'm'"
//@ diag: 18:17 note "previous declaration is here"
int h1(int x);
int h1(int x) pre(x > 0);
int h2(int x);
int h2(int x) pre(x > 0) { return x; }
struct S { void m(int a); };
void S::m(int a) pre(a > 0) {}
