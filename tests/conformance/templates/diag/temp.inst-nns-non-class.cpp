// [temp.res.general]/6, [basic.lookup.qual.general]/1: a dependent
// qualifier that is substituted with a non-class type cannot be used
// before '::'.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:52 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 9:9 note "in instantiation of template class 'S<int>' requested here"
template <class T> struct S { static const int v = T::value; };
int x = S<int>::v;
