// [dcl.stc]/1 (C++17 removed `register`, P0001R1): at namespace scope
// `register` on a variable, structured binding or variable template gets
// "ISO C++17 does not allow 'register' storage class specifier"
// (-Wregister) at the specifier and "illegal storage class on file-scoped
// variable" at the name; the variable is invalid, so its initializer is not
// checked. A member gets the member-declaration error instead (WP-293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 22:14 error "illegal storage class on file-scoped variable"
//@ diag: 23:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 23:14 error "illegal storage class on file-scoped variable"
//@ diag: 26:1 error "decomposition declaration cannot be declared 'register'"
//@ diag: 26:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 26:15 error "illegal storage class on file-scoped variable"
//@ diag: 27:15 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 27:28 error "illegal storage class on file-scoped variable"
//@ diag: 28:20 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 28:31 error "illegal storage class on file-scoped variable"
//@ diag: 29:12 error "storage class specified for a member declaration"

register int gg;
register int gi = "x";
struct P { int a, b; };
P gp{1, 2};
register auto [ga, gb] = gp;
namespace N { register int k; }
template <class T> register T vt = T();
struct S { register int m; };
