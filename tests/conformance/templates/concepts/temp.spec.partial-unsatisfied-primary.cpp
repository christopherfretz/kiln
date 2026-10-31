//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:1 error "constraints not satisfied for class template 'P' [with T = char]"
//@ diag: 7:30 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
// [temp.names]/8: a constrained primary with a partial specialization; the
// primary's constraints are still checked.
template <class T> requires (sizeof(T) > 1) struct P {};
template <class T> requires (sizeof(T) > 1) struct P<T *> {};
P<char> p;
