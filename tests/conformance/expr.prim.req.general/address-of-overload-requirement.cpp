// [expr.prim.req.simple], [over.over]: taking the address of an overload
// set or a member template with no target type is invalid, so the
// requirement is not satisfied. Found running GCC's C++ testsuite
// (WP-1497).
//@ expect: compile-ok
//@ std: c++20
struct A { template <class T> double operator()(T) const { return 0; } };
struct B { double operator()(int) const { return 0; } };
struct C { void f(int); void f(double); };
struct D { void f(int); };
template <class X> concept HasCall = requires { &X::operator(); };
template <class X> concept HasF = requires { &X::f; };
static_assert(!HasCall<A>);
static_assert(HasCall<B>);
static_assert(!HasF<C>);
static_assert(HasF<D>);
