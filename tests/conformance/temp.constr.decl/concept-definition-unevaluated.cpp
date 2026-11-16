// [temp.concept]/3, [basic.def.odr]: a concept's constraint-expression is
// an unevaluated operand, so it may name a non-static data member without
// an object. Found running GCC's C++ testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
struct A { static const int x = 42; bool y; };
template <bool V> concept C = V || A::y;
static_assert(C<true>);
template <class T> concept D = sizeof(T::y) == 1 && true;
static_assert(D<A>);
