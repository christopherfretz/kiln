// [temp.deduct.general]/8 (P4149R1, Croydon DR): the instantiation of a
// class template specialization's definition and of its default member
// initializers is outside the immediate context: an error there is a hard
// error, not a substitution failure. (GCC 16.1 accepts `nsdmi` as a
// substitution failure.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:39 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 16:51 note "in instantiation of default member initializer 'D<int>::m' requested here"
//@ diag: 18:11 note "while substituting deduced template arguments into function template 'nsdmi' [with T = int]"
//@ diag: 19:40 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 20:44 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 22:11 note "while substituting deduced template arguments into function template 'cls' [with T = int]"
//
template <class T> struct D { int m = T::value; };
template <class T> auto nsdmi(T) -> decltype(D<T>{}, 0);
long nsdmi(...);
auto r1 = nsdmi(0);
template <class T> struct C { typename T::type m; };
template <class T> auto cls(T) -> decltype(sizeof(C<T>));
long cls(...);
auto r2 = cls(0);
