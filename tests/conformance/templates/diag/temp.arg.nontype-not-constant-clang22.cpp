// [temp.arg.nontype]/1: a non-type template argument is a converted
// constant expression; a non-const variable's value is not usable in one
// ([expr.const]/5).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:3 error "non-type template argument is not a constant expression"
//@ diag: 15:3 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 14:5 note "declared here"
//@ diag: 17:3 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 16:15 note "template parameter is declared here"
//@ requires: clang>=22
// clang 22 spelling of temp.arg.nontype-not-constant.cpp (WP-377).
template <int N> struct V {};
int n = 3;
V<n> v;
template <int M> struct W {};
W<1.5> w;
