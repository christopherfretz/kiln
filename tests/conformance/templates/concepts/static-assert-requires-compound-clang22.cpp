//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion failed"
//@ diag: 12:15 note "because 'long' does not satisfy 'Cmp'"
//@ diag: 11:63 note "because 'Same<long, int>' evaluated to false"
//@ diag: 10:44 note "because '__is_same(long, int)' evaluated to false"
//@ requires: clang>=22
// clang 22 spelling of static-assert-requires-compound.cpp (WP-377).
// [expr.prim.req.compound]: an unsatisfied return-type-requirement.
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept Cmp = requires(T a) { { a + a } -> Same<int>; };
static_assert(Cmp<long>);
