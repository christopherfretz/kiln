// [temp.expl.spec]/17, [temp.inst]/2: a member template specialization
// of a class template specialization that is declared but not defined
// is incomplete: using or declaring members through it is an error.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 21:17 error "implicit instantiation of undefined template 'A<long>::B<int>'"
//@ diag: 19:57 note "template is declared here"
//@ diag: 25:27 error "implicit instantiation of undefined template 'A<short>::B<int>'"
//@ diag: 19:57 note "template is declared here"
//@ diag: 26:45 error "implicit instantiation of undefined template 'A<short>::B<int>'"
//@ diag: 19:57 note "template is declared here"
//@ diag: 27:5 error "implicit instantiation of undefined template 'A<short>::B<int>'"
//@ diag: 19:57 note "template is declared here"
//@ diag: 28:12 error "implicit instantiation of undefined template 'A<short>::B<int>'"
//@ diag: 19:57 note "template is declared here"
//@ diag: 29:30 error "implicit instantiation of undefined template 'A<short>::B<int>'"
//@ diag: 19:57 note "template is declared here"
template <class T> struct A { template <class U> struct B; };
template <> struct A<int> { template <class U> struct B {}; };
A<long>::B<int> x;
A<int>::B<int> y;
template <> template <> struct A<long>::B<int> {};
A<char>::B<char> *z;
template<> template<> int A<short>::B<int>::f() { return 0; }
template<> template<> template<class Z> int A<short>::B<int>::h() { return 3; }
int A<short>::B<int>::k = 0;
void g() { A<short>::B<int>::m(); }
template<> template<> struct A<short>::B<int>::C {};
