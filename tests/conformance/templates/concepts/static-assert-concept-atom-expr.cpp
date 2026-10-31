//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion failed"
//@ diag: 9:15 note "because 'NT' does not satisfy 'Trivial'"
//@ diag: 7:38 note "because '__is_trivially_copyable(NT)' evaluated to false"
// [temp.constr.atomic]: a non-comparison atom is shown as written.
template <class T> concept Trivial = __is_trivially_copyable(T);
struct NT { NT(const NT &); };
static_assert(Trivial<NT>);
