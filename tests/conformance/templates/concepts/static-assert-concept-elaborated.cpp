//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion failed"
//@ diag: 9:15 note "because 'int' does not satisfy 'Two'"
//@ diag: 8:34 note "because 'sizeof(int) == 2' (4 == 2) evaluated to false"
// [temp.constr.atomic]: an integral comparison is shown with its operand
// values.
template <class T> concept Two = sizeof(T) == 2;
static_assert(Two<int>);
