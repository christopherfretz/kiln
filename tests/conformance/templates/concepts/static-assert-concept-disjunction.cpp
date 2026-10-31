//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion failed"
//@ diag: 14:15 note "because 'char' does not satisfy 'BigOrPtr'"
//@ diag: 13:39 note "because 'char' does not satisfy 'Big'"
//@ diag: 11:34 note "because 'sizeof(char) > 4' (1 > 4) evaluated to false"
//@ diag: 13:49 note "and 'char' does not satisfy 'Ptr'"
//@ diag: 12:34 note "because '__is_pointer(char)' evaluated to false"
// [temp.constr.op]/3: both operands of an unsatisfied disjunction are
// explained.
template <class T> concept Big = sizeof(T) > 4;
template <class T> concept Ptr = __is_pointer(T);
template <class T> concept BigOrPtr = Big<T> || Ptr<T>;
static_assert(BigOrPtr<char>);
