//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:40 error "use of undeclared identifier 'p'"
//@ diag: 8:31 error "missing 'typename' prior to dependent type name 'T::type'"
// [temp.res.general]/5: a dependent qualified name is not a type without `typename`
template <class T> void f() { T::type *p = nullptr; }
template <class T> struct A { T::type x; };
template <class T> void g() { T::type y; }
