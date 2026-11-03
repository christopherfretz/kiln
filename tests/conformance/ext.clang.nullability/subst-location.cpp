// [temp.inst] A nullability qualifier substituted onto a non-pointer type is
// diagnosed at the qualifier, not the declaration (clang TransformAttributedType;
// spec/sema.md §4.1.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:5 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'"
//@ diag: 19:8 note "in instantiation of template class 'S<int>' requested here"
//@ diag: 17:15 error "nullability specifier '_Null_unspecified' cannot be applied to non-pointer type 'int'"
//@ diag: 21:15 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 20:25 note "candidate template ignored: substitution failure [with T = int]: nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'"
//@ diag: 22:33 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'int'"
//@ diag: 23:15 note "in instantiation of function template specialization 'g<int>' requested here"
template <class T> struct S {
  T _Nonnull m;
  int  x;
  T  * _Nullable ok;
  using U = T _Null_unspecified;
};
S<int> s;
template <class T> void f(T _Nonnull p) {}
template void f<int>(int);
template <class T> void g() { T _Nullable v; (void)v; }
template void g<int>();
