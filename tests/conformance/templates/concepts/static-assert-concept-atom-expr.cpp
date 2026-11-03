//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion failed"
//@ diag: 11:15 note "because 'NT' does not satisfy 'Trivial'"
//@ diag: 9:38 note "because '__is_trivially_copyable(NT)' evaluated to false"
//@ requires: clang<22
// WP-355 residual: clang 22 explains a failed __is_trivially_copyable atom ('NT' is not trivially copyable, user provided copy constructor, defined here); kiln lacks the trait explainer.
// [temp.constr.atomic]: a non-comparison atom is shown as written.
template <class T> concept Trivial = __is_trivially_copyable(T);
struct NT { NT(const NT &); };
static_assert(Trivial<NT>);
