// [temp.friend]: the clang 19 shape of friend-template-qualifier-not-own.cpp
// (clang 19 prints the nested-name-specifier with its trailing '::').
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:49 warning "dependent nested name specifier 'Bq<sizeof(T)>::' for friend template declaration is not supported; ignoring this friend declaration"
//@ diag: 12:52 warning "dependent nested name specifier 'Bq<sizeof(T) + 1>::' for friend template declaration is not supported; ignoring this friend declaration"
//@ diag: 13:41 warning "dependent nested name specifier 'Bq<M>::' for friend class declaration is not supported; turning off access control for 'S'"
template <int N> struct Bq { struct Z {}; template <int M> struct T {}; };
template <class T> struct S {
  template <int M> friend struct Bq<sizeof(T)>::Z;
  template <int M> friend class Bq<sizeof(T) + 1>::T;
  template <int M> friend struct Bq<M>::Z;
};
S<int> s;
