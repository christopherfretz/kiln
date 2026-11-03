// [dcl.constexpr]/1: a redeclaration is checked against the previous valid
// declaration only: a diagnosed redefinition (clang CheckForFunctionRedefinition)
// or mismatched redeclaration is invalid and not compared.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "redefinition of 'h'"
//@ diag: 13:15 note "previous definition is here"
//@ diag: 17:5 error "non-constexpr declaration of 'g' follows constexpr declaration"
//@ diag: 16:15 note "previous declaration is here"
//@ diag: 18:5 error "non-constexpr declaration of 'g' follows constexpr declaration"
//@ diag: 16:15 note "previous declaration is here"
constexpr int h() { return 0; }
constexpr int h() { return 1; }
int h();
constexpr int g();
int g();
int g();
constexpr int g();
