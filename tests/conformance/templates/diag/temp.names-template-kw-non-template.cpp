// [temp.names]/6: a name prefixed by the keyword 'template' shall be
// followed by a template argument list or refer to a class template or an
// alias template; it must name a template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:68 error "'n' following the 'template' keyword does not refer to a template"
//@ diag: 11:15 note "in instantiation of static data member 'B<A>::m' requested here"
//@ diag: 9:33 note "declared as a non-template here"
struct A { static constexpr int n = 1; };
template <class T> struct B { static constexpr int m = T::template n<int>; };
int x = B<A>::m;
