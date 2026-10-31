//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "constraints not satisfied for variable template 'four' [with T = char]"
//@ diag: 7:30 note "because 'sizeof(char) == 4' (1 == 4) evaluated to false"
// [temp.names]/8: a variable template specialization whose constraints are
// not satisfied.
template <class T> requires (sizeof(T) == 4) constexpr int four = 4;
int x = four<char>;
