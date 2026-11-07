// [dcl.contract.func]/7 (P2900R14): the const rule on a non-reference
// parameter a postcondition odr-uses holds on every declaration of the
// function, also on a friend redeclaration (or definition) inside a
// class that omits the specifier sequence. GCC 16.1 rejects these too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 15:30 error "a value parameter used in a postcondition must be const"
//@ diag: 15:30 note "declared here"
//@ diag: 17:29 error "a value parameter used in a postcondition must be const"
//@ diag: 17:29 note "declared here"
int f(const int a) post(r: r == a);
struct S0 { friend int f(const int a); };
struct S1 { friend int f(int a); };
int h(const int a) post(r: r == a);
struct U { friend int h(int a) { return a; } };
int k(const int a, int b) post(r: r == a);
struct V { friend int k(const int a, int b); };
