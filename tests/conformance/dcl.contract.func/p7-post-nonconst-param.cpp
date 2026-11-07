// [dcl.contract.func]/7 (P2900R14): a non-reference parameter that a
// postcondition odr-uses is const on every declaration of the function,
// also on one without the specifier sequence; a function template's is
// checked at instantiation. GCC 16.1 rejects these too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 16:12 error "a value parameter used in a postcondition must be const"
//@ diag: 16:12 note "declared here"
//@ diag: 18:12 error "a value parameter used in a postcondition must be const"
//@ diag: 18:12 note "declared here"
//@ diag: 20:27 error "a value parameter used in a postcondition must be const"
//@ diag: 21:20 note "in instantiation of function template specialization 'f4<int>' requested here"
//@ diag: 20:27 note "declared here"
int f1(int x) post(r: r > x);
int f2(const int x) post(r: r > x);
int f2(int x);
int f3(int& x, const int y) post(r: r > x + y);
template <class T> T f4(T t) post(r: r >= t) { return t; }
int use() { return f4(1); }
