// [class.derived]/2, [temp.inst]/3: a class whose base class
// specialization instantiated invalid (a member typedef failed to
// substitute) is invalid as well, so naming a member of it after the
// error is quiet, through any number of derivation levels; a valid
// specialization is unaffected.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:20 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 18:31 note "in instantiation of template class 'bad<int>' requested here"
//@ diag: 20:16 note "in instantiation of template class 'D<int>' requested here"
//@ diag: 16:20 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 18:31 note "in instantiation of template class 'bad<long>' requested here"
//@ diag: 19:32 note "in instantiation of template class 'D<long>' requested here"
//@ diag: 21:16 note "in instantiation of template class 'D2<long>' requested here"
template <class T> struct bad {
  typedef typename T::nope type;
};
template <class T> struct D : bad<T> {};
template <class T> struct D2 : D<T> {};
int x = sizeof(D<int>::type);
int y = sizeof(D2<long>::type);
struct S { typedef int nope; };
int z = sizeof(D2<S>::type);
