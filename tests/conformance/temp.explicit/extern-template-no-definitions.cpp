// [temp.explicit]/12: after an explicit instantiation declaration of a
// class, the definitions of its non-inline member functions and static
// data members are not implicitly instantiated, even when odr-used, so
// definitions that would be ill-formed for the arguments are accepted
// (clang `InstantiateFunctionDefinition` / `InstantiateVariableDefinition`
// return early for `TSK_ExplicitInstantiationDeclaration`). Members of a
// nested class are covered by the declaration too.
//@ expect: compile-ok
//@ strict-diags
template <class T> struct X {
  void f();
  static int s;
  struct N { void h(); };
  virtual void v();
  inline void i();
  constexpr int c() const;
};
template <class T> void X<T>::f() { T::error1; }
template <class T> int X<T>::s = T::error2;
template <class T> void X<T>::N::h() { T::error3; }
template <class T> void X<T>::v() { T::error4; }
template <class T> inline void X<T>::i() {}
template <class T> constexpr int X<T>::c() const { return sizeof(T); }
extern template struct X<int>;
static_assert(X<int>{}.c() == sizeof(int));
void use(X<int>& x, X<int>::N& n) { x.f(); n.h(); x.v(); x.i(); (void)X<int>::s; }
