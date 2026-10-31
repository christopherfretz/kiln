//@ expect: compile-ok
// [expr.prim.req.simple]: a simple-requirement asserts the validity of an
// expression.
template <class T> concept Addable = requires(T a, T b) { a + b; };
struct S {};
static_assert(Addable<int>);
static_assert(Addable<double *> == false);
static_assert(!Addable<S>);
