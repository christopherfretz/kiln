//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:15 error "static assertion failed"
//@ diag: 8:15 note "because 'int' does not satisfy 'Nest'"
//@ diag: 7:55 note "because 'sizeof(int) == 2' (4 == 2) evaluated to false"
// [expr.prim.req.nested]: an unsatisfied nested-requirement.
template <class T> concept Nest = requires { requires sizeof(T) == 2; };
static_assert(Nest<int>);
