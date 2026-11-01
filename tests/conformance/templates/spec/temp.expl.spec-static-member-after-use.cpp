// [temp.expl.spec]/7: a static data member explicitly specialized after
// its use implicitly instantiated it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:26 error "explicit specialization of 's' after instantiation"
//@ diag: 9:27 note "implicit instantiation first required here"
template <class T> struct A { static int s; };
template <class T> int A<T>::s = 1;
int u() { return A<long>::s; }
template <> int A<long>::s = 9;
