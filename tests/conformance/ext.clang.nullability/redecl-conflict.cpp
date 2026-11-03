// [dcl.fct] clang -Wnullability: a redeclaration's parameter whose nullability
// differs from the previous declaration's (inherited when unwritten); return
// types are not compared (spec/sema.md §4.1.1).
//@ strict-diags
//@ diag: 18:25 warning "nullability specifier '_Nullable' conflicts with existing specifier '_Nonnull'"
//@ diag: 17:24 note "previous declaration is here"
//@ diag: 28:36 warning "nullability specifier '_Nullable' conflicts with existing specifier '_Nonnull'"
//@ diag: 26:35 note "previous declaration is here"
//@ diag: 30:48 warning "nullability specifier '_Null_unspecified' conflicts with existing specifier '_Nonnull'"
//@ diag: 29:39 note "previous declaration is here"
//@ diag: 34:49 warning "nullability specifier '_Nonnull' conflicts with existing specifier '_Nullable'"
//@ diag: 33:49 note "previous declaration is here"
//@ diag: 41:24 warning "nullability specifier '_Nullable' conflicts with existing specifier '_Nonnull'"
//@ diag: 40:14 note "previous declaration is here"
int * _Nonnull f1();
int * _Nullable f1();
void f2(int * _Nonnull p);
void f2(int * _Nullable p);
void f3(int * _Nonnull p);
void f3(int * p);
void f4(int * p);
void f4(int * _Nonnull p);
int * _Nonnull f5();
int * f5();
struct S {
  int * _Nullable m(int * _Nonnull);
};
int * _Nonnull S::m(int * _Nullable) { static int i; return &i; }
void f6(int * _Nonnull, int * _Nonnull);
void f6(int * _Nonnull, int * _Null_unspecified);
int * _Null_unspecified f7();
int * _Nonnull f7();
template <class T> T * _Nonnull f8(T * _Nullable);
template <class T> T * _Nullable f8(T * _Nonnull);
void f9(int * _Nonnull * _Nullable p);
void f9(int * _Nullable * _Nullable p);
int * _Nullable_result f10();
int * _Nullable f10();
void g2(int *_Nonnull p);
void g2(int *p);
void g2(int *_Nullable p);
