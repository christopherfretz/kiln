// [dcl.stc]/1 (C++17 removed `register`, P0001R1): at namespace scope
// `register` on a variable, structured binding or variable template gets
// "ISO C++17 does not allow 'register' storage class specifier"
// (-Wregister) at the specifier and "illegal storage class on file-scoped
// variable" at the name; the variable is invalid, so its initializer is not
// checked. A member gets the member-declaration error instead (WP-293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 24:14 error "illegal storage class on file-scoped variable"
//@ diag: 25:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 25:14 error "illegal storage class on file-scoped variable"
//@ diag: 28:1 error "declaration cannot be declared 'register'"
//@ diag: 28:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 28:15 error "illegal storage class on file-scoped variable"
//@ diag: 29:15 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 29:28 error "illegal storage class on file-scoped variable"
//@ diag: 30:20 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 30:31 error "illegal storage class on file-scoped variable"
//@ diag: 31:12 error "storage class specified for a member declaration"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).

register int gg;
register int gi = "x";
struct P { int a, b; };
P gp{1, 2};
register auto [ga, gb] = gp;
namespace N { register int k; }
template <class T> register T vt = T();
struct S { register int m; };
