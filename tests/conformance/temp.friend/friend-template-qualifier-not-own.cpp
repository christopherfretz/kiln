// [temp.friend]: a friend template whose qualifier is a dependent
// specialization of another template (Bq<sizeof(T)>) cannot be matched to a
// member template; it is ignored with a warning, as clang does. A friend class
// whose qualifier depends on the friend's own parameter (Bq<M>) keeps the
// access-control warning. Clang 22 prints the specifier without its trailing
// '::' (the -19 twin has the clang 19 shape). Reduced from real code.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:49 warning "dependent nested name specifier 'Bq<sizeof(T)>' for friend template declaration is not supported; ignoring this friend declaration"
//@ diag: 16:52 warning "dependent nested name specifier 'Bq<sizeof(T) + 1>' for friend template declaration is not supported; ignoring this friend declaration"
//@ diag: 17:41 warning "dependent nested name specifier 'Bq<M>' for friend class declaration is not supported; turning off access control for 'S'"
template <int N> struct Bq { struct Z {}; template <int M> struct T {}; };
template <class T> struct S {
  template <int M> friend struct Bq<sizeof(T)>::Z;
  template <int M> friend class Bq<sizeof(T) + 1>::T;
  template <int M> friend struct Bq<M>::Z;
};
S<int> s;
