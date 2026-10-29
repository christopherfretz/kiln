// [temp.spec.partial]/8, [temp.deduce.type]/17: non-type arguments in
// partial specializations, including a deduced non-type parameter.
template <int N, bool Even = N % 2 == 0> struct parity { static constexpr int v = 1; };
template <int N> struct parity<N, true> { static constexpr int v = 0; };
template <> struct parity<0, true> { static constexpr int v = 2; };
static_assert(parity<3>::v == 1);
static_assert(parity<4>::v == 0);
static_assert(parity<0>::v == 2);
template <class T, T V> struct constant { static constexpr T value = V; };
template <class T> struct is_zero { static constexpr bool value = false; };
template <class T> struct is_zero<constant<T, 0>> { static constexpr bool value = true; };
static_assert(is_zero<constant<int, 0>>::value);
static_assert(!is_zero<constant<int, 1>>::value);
