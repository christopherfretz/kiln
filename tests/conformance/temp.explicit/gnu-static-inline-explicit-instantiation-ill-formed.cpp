// The GNU keyword before an explicit instantiation is taken only alone and
// at namespace scope (clang 22): `static inline template` and a
// block-scope `static template` are errors (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 10:15 error "expected unqualified-id"
//@ diag: 11:19 error "expected unqualified-id"
template<class T> struct A {};
static inline template struct A<int>;
void f() { static template struct A<char>; }
