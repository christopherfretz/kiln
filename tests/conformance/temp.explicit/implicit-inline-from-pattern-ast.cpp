//@ expect: ast
//@ ast-filter: X
// [dcl.inline]/4, [temp.explicit]/12: an instantiated member function is
// implicitly inline only when its pattern is (defined in the class or
// declared `inline`); an out-of-line non-inline pattern gives a non-inline
// instantiation (clang `InstantiateFunctionDefinition` copies
// `isInlined`). An explicit instantiation declaration of the class also
// instantiates its member classes (clang `InstantiateClassMembers`).
template <class T> struct X {
  T inl();
  T non();
  T in() { return 0; }
  struct N { T t; };
};
template <class T> inline T X<T>::inl() { return 1; }
template <class T> T X<T>::non() { return 2; }
int u(X<int> x) { return x.inl() + x.non() + x.in(); }
extern template struct X<long>;
