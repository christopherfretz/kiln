//@ expect: compile-ok
// [expr.prim.req.compound]/1: the expression is valid and its
// decltype((E)) satisfies the return-type-requirement's type-constraint.
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept IntPlus = requires(T a) { { a + 1 } -> Same<int>; };
template <class T> concept RefDeref = requires(T p) { { *p } -> Same<int &>; };
static_assert(IntPlus<int>);
static_assert(IntPlus<char>);
static_assert(!IntPlus<long>);
static_assert(RefDeref<int *>);
static_assert(!RefDeref<const int *>);
