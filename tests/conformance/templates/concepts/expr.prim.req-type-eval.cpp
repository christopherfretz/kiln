//@ expect: compile-ok
// [expr.prim.req.type]: a type-requirement asserts the validity of a type.
template <class T> concept HasType = requires { typename T::type; };
template <class T> concept HasTmpl = requires { typename T::template rebind<int>; };
struct A { using type = int; };
struct B { template <class U> using rebind = U; };
static_assert(HasType<A>);
static_assert(!HasType<B>);
static_assert(!HasType<int>);
static_assert(HasTmpl<B>);
static_assert(!HasTmpl<A>);
