//@ expect: compile-ok
// [temp.spec.partial.order]: constrained partial specializations are
// ordered by their constraints.
template <class T> concept Int = __is_same(T, int);
template <class T> concept Arith = Int<T> || __is_same(T, double);
template <class T> struct X { static constexpr int v = 0; };
template <Arith T> struct X<T> { static constexpr int v = 1; };
template <Int T> struct X<T> { static constexpr int v = 2; };
static_assert(X<char>::v == 0);
static_assert(X<double>::v == 1);
static_assert(X<int>::v == 2);
