// [temp.expl.spec], [temp.names]/8: an explicit specialization of a member
// whose trailing requires-clause the class's arguments do not satisfy
// matches no member, and a constrained variable template's arguments must
// satisfy its constraints. Found running GCC's C++ testsuite.
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:25 error "out-of-line definition of 'f' does not match any declaration in 'S<int>'"
//@ diag: 19:26 note "S defined here"
//@ diag: 20:8 note "member declaration nearly matches"
//@ diag: 24:26 error "out-of-line definition of 'g' does not match any declaration in 'S<char>'"
//@ diag: 19:26 note "S defined here"
//@ diag: 21:8 note "member declaration nearly matches"
//@ diag: 27:16 error "constraints not satisfied for variable template 'v' [with T = char]"
//@ diag: 26:10 note "because 'Same<char, int>' evaluated to false"
//@ diag: 25:43 note "because '__is_same(char, int)' evaluated to false"
template<class T> concept Cls = __is_class(T);
template<class T> struct S {
  void f() requires Cls<T>;
  void g() requires (sizeof(T) > 1);
};
template<> void S<int>::f() {}
template<> void S<char>::g() {}
template<class T, class U> concept Same = __is_same(T, U);
template<Same<int> T> int v = 0;
template<> int v<char> = 1;
