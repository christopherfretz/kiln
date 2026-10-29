// [temp.spec.partial.match]/2, [temp.deduce.type]/2: a parameter used
// twice must deduce the same argument both times.
template <class T, class U> struct is_same { static constexpr bool value = false; };
template <class T> struct is_same<T, T> { static constexpr bool value = true; };
static_assert(is_same<int, int>::value);
static_assert(!is_same<int, const int>::value);
static_assert(!is_same<int, long>::value);
static_assert(is_same<int *, int *>::value);
template <class T, class U> struct pair_of { static constexpr int v = 0; };
template <class T> struct pair_of<T *, T> { static constexpr int v = 1; };
static_assert(pair_of<int *, int>::v == 1);
static_assert(pair_of<int *, long>::v == 0);
