//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion failed"
//@ diag: 9:15 note "because 'S' does not satisfy 'Addable'"
//@ diag: 8:56 note "because 'a + a' would be invalid: invalid operands to binary expression ('S' and 'S')"
// [expr.prim.req.simple]: an invalid expression in a simple-requirement.
struct S {};
template <class T> concept Addable = requires(T a) { a + a; };
static_assert(Addable<S>);
