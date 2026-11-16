// [temp.res.general]/4-5: outside the type-only contexts a qualified name
// with a dependent qualifier is not a type without `typename`: a
// block-scope declaration (`T::type x;` and `T::type operator+(int);`,
// an expression there) and a parameter of a namespace-scope function whose
// declarator-id is unqualified (WP-1495).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:30 error "missing 'typename' prior to dependent type name 'T::type'"
//@ diag: 14:37 error "expected ';' after expression"
//@ diag: 14:51 error "expected '(' for function-style cast or type construction"
//@ diag: 15:26 error "missing 'typename' prior to dependent type name 'T::type'"
template<class T> void f() { T::type x; (void)x; }
template<class T> void h() { T::type operator+(int); }
template<class T> void k(T::type x);
