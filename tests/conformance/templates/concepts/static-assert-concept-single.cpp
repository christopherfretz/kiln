//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion failed"
//@ diag: 9:15 note "because 'char' does not satisfy 'Big'"
//@ diag: 8:34 note "because 'sizeof(char) > 4' (1 > 4) evaluated to false"
// [temp.constr.atomic]: a false atomic constraint of a single-argument
// concept.
template <class T> concept Big = sizeof(T) > 4;
static_assert(Big<char>);
