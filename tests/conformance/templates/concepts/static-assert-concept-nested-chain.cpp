//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion failed"
//@ diag: 11:15 note "because 'char' does not satisfy 'Outer'"
//@ diag: 10:48 note "because 'char' does not satisfy 'Big'"
//@ diag: 9:34 note "because 'sizeof(char) > 4' (1 > 4) evaluated to false"
// [temp.constr.normal]: a concept-id in a concept explains its own
// constraint.
template <class T> concept Big = sizeof(T) > 4;
template <class T> concept Outer = Big<T *> && Big<T>;
static_assert(Outer<char>);
