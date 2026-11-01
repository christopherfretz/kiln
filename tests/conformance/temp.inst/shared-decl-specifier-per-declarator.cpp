// [temp.inst]/3, [dcl.dcl]/3: each declarator of a member declaration is
// instantiated on its own, so an error in the shared decl-specifier is
// reported once per declarator (clang 19), the instantiation context only
// with the first; a second specialization reports again.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:20 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 32:8 note "in instantiation of template class 'S<int>' requested here"
//@ diag: 25:20 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 27:19 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 27:19 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 28:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 28:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:22 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 33:30 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:8 note "in instantiation of template class 'R<int>' requested here"
//@ diag: 33:30 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 33:30 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 35:9 note "in instantiation of template class 'R<long>' requested here"
//@ diag: 33:30 error "type 'long' cannot be used prior to '::' because it has no members"
template<class T> struct S {
  typedef typename T::type g, (*h)(typename T::type);
  typename T::type a, b, *c;
  static typename T::type s1, s2;
  typename T::type f1(), f2(int);
  using U = typename T::type;
  U u1, u2;
};
S<int> s;
template<class T> struct R { T::type x, y; };
R<int> r;
R<long> r2;
