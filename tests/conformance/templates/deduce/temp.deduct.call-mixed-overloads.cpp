// [over.match.best]/2.4: a non-template beats a specialization with the same conversions.
template <class T> constexpr int f(T) { return 1; }
constexpr int f(int) { return 2; }
static_assert(f(1) == 2);
static_assert(f(1L) == 1);
static_assert(f<>(1) == 1);
