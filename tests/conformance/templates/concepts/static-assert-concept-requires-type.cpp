//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion failed"
//@ diag: 13:15 note "because 'const int *' does not satisfy 'Deref'"
//@ diag: 12:62 note "because type constraint 'Same<const int &, int &>' was not satisfied:"
//@ diag: 11:44 note "because '__is_same(const int &, int &)' evaluated to false"
//@ requires: clang<22
// clang 19 spelling; static-assert-concept-requires-type-clang22.cpp has clang 22's (WP-377).
// [expr.prim.req.compound]: a return-type-requirement over a type that
// fails.
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept Deref = requires(T p) { { *p } -> Same<int &>; };
static_assert(Deref<const int *>);
