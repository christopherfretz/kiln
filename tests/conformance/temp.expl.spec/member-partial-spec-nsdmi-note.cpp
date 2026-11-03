// [temp.expl.spec]/2, [temp.spec.partial.member]/2, [class.mem.general]/11:
// the default member initializer of a member partial specialization of an
// explicitly specialized class template specialization is instantiated from
// that partial specialization, not the primary member template; its error
// is reported there and the instantiation note points at the class name
// (the implicit default constructor uses it).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:71 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 16:47 note "in instantiation of default member initializer 'Y<int>::M<int *>::u' requested here"
//@ diag: 17:18 note "in evaluation of exception specification for 'Y<int>::M<int *>::M' needed here"
template <class T> struct Y {
  template <class U> struct M { U u = 0; };
};

template <> template <class U> struct Y<int>::M<U *> { int pad; U u = U::nope; };
Y<int>::M<int *> m;
