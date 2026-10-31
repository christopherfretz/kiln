//@ expect: compile-ok
// [temp.spec.partial.match]: a partial specialization matches only when its
// constraints are satisfied.
template <class T> struct P { static constexpr int v = 0; };
template <class T> requires (sizeof(T) == 1) struct P<T *> { static constexpr int v = 1; };
static_assert(P<char *>::v == 1);
static_assert(P<int *>::v == 0);
static_assert(P<int>::v == 0);
