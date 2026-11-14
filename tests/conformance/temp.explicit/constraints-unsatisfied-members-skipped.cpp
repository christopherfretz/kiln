// [temp.explicit]/10: an explicit instantiation of a class template
// specialization explicitly instantiates each direct non-template member
// only if the member's associated constraints are satisfied by the
// template arguments; a member whose trailing requires-clause is not
// satisfied is skipped with no diagnostic (clang `InstantiateClassMembers`
// `CheckFunctionConstraints`). Covers member functions defined in and out
// of the class, constructors, conversion functions, the members of a
// nested class (instantiated transitively), an explicit instantiation
// declaration followed by a definition, and a friend defined in the class
// (not a member, never instantiated by it). Found building an open-source
// linker.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template <typename E> concept has_abi = requires { E::abi; };
struct A { static constexpr int abi = 1; };
struct B {};
template <typename E> struct File {
  int f() requires has_abi<E>;
  int g() { return 0; }
  File() requires has_abi<E> { (void)E::abi; }
  File(int) {}
  operator int() requires has_abi<E> { return E::abi; }
  struct N {
    int h() requires has_abi<E> { return E::abi; }
    int k() { return 1; }
  };
  template <class U> int t(U) requires has_abi<E> { return E::abi; }
  friend int fr(File) requires has_abi<E> { return E::abi; }
};
template <typename E> int File<E>::f() requires has_abi<E> { return E::abi; }
template struct File<A>;
template struct File<B>;
template <typename E> struct Ext {
  int f() requires has_abi<E> { return E::abi; }
  int g() { return 2; }
};
extern template struct Ext<B>;
int use(Ext<B> &x) { return x.g(); }
template struct Ext<B>;
int calls(File<A> &a, File<B> &b) { return a.f() + int(a) + b.g() + fr(a); }
