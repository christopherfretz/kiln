// [over.match.best]/2.1: better conversions decide before partial ordering.
template <class T> constexpr int f(T, long) { return 1; }
template <class T> constexpr int f(T*, int) { return 2; }
int x;
static_assert(f(&x, 1L) == 1);
static_assert(f(&x, 1) == 2);
