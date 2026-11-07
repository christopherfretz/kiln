// [dcl.contract.func]/4-5 (P2900R14): a redeclaration that has a
// function-contract-specifier-seq has the same one as the first
// declaration: the same assertions in the same order with equivalent
// predicates. GCC 16.1 rejects these too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 16:15 error "function contract specifiers of 'h1' do not match the previous declaration"
//@ diag: 15:5 note "previous declaration is here"
//@ diag: 18:15 error "function contract specifiers of 'h2' do not match the previous declaration"
//@ diag: 17:5 note "previous declaration is here"
//@ diag: 20:15 error "function contract specifiers of 'h3' do not match the previous declaration"
//@ diag: 19:5 note "previous declaration is here"
int h1(int x) pre(x);
int h1(int x) pre(x > 1);
int h2(int x) pre(x > 0) post(r: r > 0);
int h2(int x) post(r: r > 0) pre(x > 0);
int h3(int x) pre(x > 0) pre(x < 9);
int h3(int x) pre(x > 0) { return x; }
int h4(int x) pre(x > 0);
int h4(int y) pre(y > 0);
