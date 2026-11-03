// [temp.explicit]/10,12: an explicit instantiation declaration of a class
// instantiates its member classes, which take the declaration kind, and
// leaves the definitions of its non-inline members (also the nested
// class's) uninstantiated even when used: they are external declarations.
// A later explicit instantiation definition upgrades such a member class
// to a definition, so its vtable, RTTI and members are emitted `weak_odr`
// (clang `InstantiateClassMembers`, `MarkVTableUsed`).
template <class T> struct X {
  void f();
  static int s;
  struct N { void h(); virtual void k(); };
  virtual void v();
};
template <class T> void X<T>::f() {}
template <class T> int X<T>::s = 3;
template <class T> void X<T>::N::h() {}
template <class T> void X<T>::N::k() {}
template <class T> void X<T>::v() {}
extern template struct X<int>;
int use(X<int>& x, X<int>::N& n) { x.f(); n.h(); x.v(); return X<int>::s + sizeof(X<int>::N); }
template struct X<int>;
extern template struct X<long>;
int use2(X<long>& x, X<long>::N& n) { x.f(); n.h(); return X<long>::s; }
