// [temp.res.general]/3, [temp.inst]: an invalid dependent-name type
// (`typename T::type`, `typename T::template X<int>`, a nested
// `T::N::type`) is diagnosed at its written nested-name-specifier in
// the instantiated member, alias, typedef and function signature.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 28:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 20:19 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 21:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 22:19 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 23:22 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 24:20 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 25:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 32:9 error "no matching function for call to 'k'"
//@ diag: 31:36 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used prior to '::' because it has no members"
template<class T> struct A {
  typename T::type x;
  static typename T::type y;
  typename T::type f();
  void g(typename T::type);
  using U = typename T::type;
  typedef typename T::type V;
  typename T::template X<int> w;
  typename T::N::type q;
};
A<int> a;
template<class T> struct D { struct N { }; };
template<class T> void h(typename T::type p) {}
template<class T> typename T::type k() { return 0; }
int z = k<int>();
