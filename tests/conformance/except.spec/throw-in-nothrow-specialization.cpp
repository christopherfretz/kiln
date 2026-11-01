// -Wexceptions in an instantiated function template names the specialization
// unqualified with its arguments ('k<int>'); a member of a class template
// specialization is named plainly ('~S').
//@ expect: compile-ok
//@ strict-diags
//@ diag: 18:39 warning "'k<int>' has a non-throwing exception specification but can still throw"
//@ diag: 21:12 note "in instantiation of function template specialization 'k<int>' requested here"
//@ diag: 18:24 note "function declared non-throwing here"
//@ diag: 19:37 warning "'~S' has a non-throwing exception specification but can still throw"
//@ diag: 21:29 note "in instantiation of member function 'S<int>::~S' requested here"
//@ diag: 19:30 note "destructor has a implicit non-throwing exception specification"
//@ diag: 19:88 warning "'m<char>' has a non-throwing exception specification but can still throw"
//@ diag: 21:34 note "in instantiation of function template specialization 'S<int>::m<char>' requested here"
//@ diag: 19:73 note "function declared non-throwing here"
//@ diag: 20:60 warning "'q<long, 3>' has a non-throwing exception specification but can still throw"
//@ diag: 21:48 note "in instantiation of function template specialization 'n::q<long, 3>' requested here"
//@ diag: 20:45 note "function declared non-throwing here"
template<class T> void k() noexcept { throw T(); }
template<class T> struct S { ~S() { throw T(); } template<class U> void m() noexcept { throw U(); } };
namespace n { template<class T, int N> void q() noexcept { throw N; } }
void u() { k<int>(); S<int> s; s.m<char>(); n::q<long, 3>(); }
