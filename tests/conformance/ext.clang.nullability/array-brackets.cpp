// [dcl.array] clang's bracket declarator: a nullability qualifier in a
// parameter's outermost array brackets moves to the decayed pointer
// ([dcl.fct]/5), elsewhere it names a non-pointer; C99 'static' and
// qualifiers there are errors in C++ (spec/sema.md §4.1.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:13 error "static array size is a C99 feature, not permitted in C++"
//@ diag: 29:7 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int[3]'"
//@ diag: 31:17 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int[]'"
//@ diag: 31:13 error "array has incomplete element type 'int[]'"
//@ diag: 32:17 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int[]'"
//@ diag: 33:13 error "qualifier in array size is a C99 feature, not permitted in C++"
//@ diag: 34:23 error "nullability specifier '_Nullable' conflicts with existing specifier '_Nonnull'"
//@ diag: 36:18 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int[2]'"
//@ diag: 37:6 error "type qualifier used in array declarator outside of function prototype"
//@ diag: 38:13 error "static array size is a C99 feature, not permitted in C++"
//@ diag: 39:13 error "qualifier in array size is a C99 feature, not permitted in C++"
//@ diag: 40:6 error "'static' used in array declarator outside of function prototype"
//@ diag: 41:13 error "static array size is a C99 feature, not permitted in C++"
//@ diag: 42:13 error "static array size is a C99 feature, not permitted in C++"
//@ diag: 43:13 error "qualifier in array size is a C99 feature, not permitted in C++"
//@ diag: 44:20 warning "duplicate 'const' declaration specifier"
//@ diag: 44:13 error "qualifier in array size is a C99 feature, not permitted in C++"
//@ diag: 46:12 warning "nullability specifier '_Nonnull' conflicts with existing specifier '_Nullable'"
//@ diag: 39:12 note "previous declaration is here"
void f(int a[_Nonnull]);
void g(int a[_Nullable 4]);
void h(int a[static _Nonnull 3]);
int x[_Nonnull 3];
void k(int b[_Nonnull][4]);
void m(int c[4][_Nonnull]);
void n(int (*d)[_Nonnull]);
void p(int e[const _Nonnull]);
void q(int e[_Nonnull _Nullable]);
void r(int *a[_Nonnull]);
void s() { int z[_Nonnull 2]; }
int x[const 3];
void f(int a[static 3]);
void g(int a[volatile]);
int y[static 2];
void h(int a[const static 3]);
void k(int a[static const _Nonnull 3]);
void m(int a[_Nonnull const]);
void n(int a[const const]);
void f(int *_Nonnull a);
void g(int a[_Nonnull]) { static_assert(__is_same(decltype(a), int * _Nonnull)); }
template <class T> void t(T a[_Nullable]);
void u(int a[_Nonnull 3], int b[_Null_unspecified]);
