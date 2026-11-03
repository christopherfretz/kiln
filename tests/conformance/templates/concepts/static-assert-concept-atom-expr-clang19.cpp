//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion failed"
//@ diag: 11:15 note "because 'NT' does not satisfy 'Trivial'"
//@ diag: 9:38 note "because '__is_trivially_copyable(NT)' evaluated to false"
//@ requires: clang<22
// clang 22 adds the trait's explanation: static-assert-concept-atom-expr.cpp.
// [temp.constr.atomic]: a non-comparison atom is shown as written.
template <class T> concept Trivial = __is_trivially_copyable(T);
struct NT { NT(const NT &); };
static_assert(Trivial<NT>);
