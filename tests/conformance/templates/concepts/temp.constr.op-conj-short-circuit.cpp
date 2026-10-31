//@ expect: compile-ok
// [temp.constr.op]/2: a conjunction is not satisfied if its left operand is
// not; the right operand is then not substituted into.
template <class T> concept HasType = requires { typename T::type; };
template <class T> concept Inner = HasType<T> && sizeof(typename T::type) > 1;
struct A { using type = int; };
static_assert(Inner<A>);
static_assert(!Inner<int>);
