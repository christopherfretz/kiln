// [temp.spec.partial.match]/1, [temp.expl.spec]: an explicit
// specialization is used for its arguments even when a partial
// specialization also matches them.
template <class T> struct tag { static constexpr int v = 0; };
template <class T> struct tag<T *> { static constexpr int v = 1; };
template <> struct tag<int *> { static constexpr int v = 2; };
static_assert(tag<int>::v == 0);
static_assert(tag<long *>::v == 1);
static_assert(tag<int *>::v == 2);
