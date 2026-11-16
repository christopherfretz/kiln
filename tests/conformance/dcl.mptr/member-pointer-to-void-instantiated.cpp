// [dcl.mptr]/3: no pointer to member of reference or cv void type, also
// after substitution. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 11:36 error "'p' declared as a member pointer to void"
//@ diag: 12:9 note "in instantiation of template class 'P<void>' requested here"
//@ diag: 11:36 error "'p' declared as a member pointer to a reference of type 'int &'"
//@ diag: 13:10 note "in instantiation of template class 'P<int &>' requested here"
struct A {};
template <class T> struct P { T A::*p; };
P<void> pv;
P<int &> pr;
// Valid.
P<int> pi;
