//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion failed"
//@ diag: 12:15 note "because 'long long' does not satisfy 'SmallPtr'"
//@ diag: 11:39 note "because 'long long' does not satisfy 'Small'"
//@ diag: 9:36 note "because 'sizeof(long long) < 4' (8 < 4) evaluated to false"
// [temp.constr.op]/2: only the first unsatisfied operand of a conjunction is
// explained.
template <class T> concept Small = sizeof(T) < 4;
template <class T> concept Ptr = __is_pointer(T);
template <class T> concept SmallPtr = Small<T> && Ptr<T>;
static_assert(SmallPtr<long long>);
