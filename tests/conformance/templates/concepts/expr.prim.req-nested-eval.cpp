//@ expect: compile-ok
// [expr.prim.req.nested]: a nested-requirement is satisfied when its
// constraint-expression is.
template <class T> concept Two = requires { requires sizeof(T) == 2; };
template <class T> concept Deep = requires(T t) { requires requires { t + t; }; };
static_assert(Two<short>);
static_assert(!Two<int>);
static_assert(Deep<int>);
struct S {};
static_assert(!Deep<S>);
