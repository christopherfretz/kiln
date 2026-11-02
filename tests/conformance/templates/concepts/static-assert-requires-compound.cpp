//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion failed"
//@ diag: 12:15 note "because 'long' does not satisfy 'Cmp'"
//@ diag: 11:63 note "because type constraint 'Same<long, int>' was not satisfied:"
//@ diag: 10:44 note "because '__is_same(long, int)' evaluated to false"
//@ requires: clang<22
// clang 22 emits error: static assertion failed; WP-355 residual
// [expr.prim.req.compound]: an unsatisfied return-type-requirement.
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept Cmp = requires(T a) { { a + a } -> Same<int>; };
static_assert(Cmp<long>);
