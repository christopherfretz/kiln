//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:1 error "constraints not satisfied for class template 'S' [with T = char]"
//@ diag: 10:29 note "because 'char' does not satisfy 'Big'"
//@ diag: 8:34 note "because 'sizeof(char) > 4' (1 > 4) evaluated to false"
// [temp.names]/8: a class template specialization whose constraints are not
// satisfied.
template <class T> concept Big = sizeof(T) > 4;
template <class T> concept HasX = requires(T t) { t.x; };
template <class T> requires Big<T> && HasX<T> struct S {};
S<char> s;
