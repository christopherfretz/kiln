//@ expect: ast
//@ ast-filter: UT
//@ requires: clang>=22
// [namespace.udecl]/1, [temp.res.general]/5: a `using typename` declarator
// with a dependent qualifier declares an UnresolvedUsingTypenameDecl; a
// use of its name is the dependent type `UnresolvedUsingType`. The
// instantiation redoes the using-declaration: a UsingDecl with the
// UsingShadowDecl of the type it names.
template<class T> struct UTBase { using type = T; };
template<class T> struct UTDerived : UTBase<T> {
  using typename UTBase<T>::type;
  type m;
  type get() const { return m; }
};
UTDerived<int> ut_d;
int ut_get() { return ut_d.get(); }
