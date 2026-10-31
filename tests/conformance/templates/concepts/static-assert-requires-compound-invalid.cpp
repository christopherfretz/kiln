//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion failed"
//@ diag: 9:15 note "because 'int *' does not satisfy 'Cmp'"
//@ diag: 8:54 note "because 'a + a' would be invalid: invalid operands to binary expression ('int *' and 'int *')"
// [expr.prim.req.compound]: an invalid expression in a compound-requirement.
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept Cmp = requires(T a) { { a + a } -> Same<int>; };
static_assert(Cmp<int *>);
