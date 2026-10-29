// [temp.deduce.type]/8, [temp.spec.partial.match]: array bounds and
// element types deduced in class template partial specializations.
template <class T> struct extent { static constexpr unsigned long value = 0; };
template <class T, unsigned long N> struct extent<T[N]> { static constexpr unsigned long value = N; };
template <class T> struct remove_extent { using type = T; };
template <class T> struct remove_extent<T[]> { using type = T; };
template <class T, unsigned long N> struct remove_extent<T[N]> { using type = T; };
static_assert(extent<int>::value == 0);
static_assert(extent<int[4]>::value == 4);
static_assert(extent<int[4][5]>::value == 4);
static_assert(sizeof(remove_extent<int[4][5]>::type) == 5 * sizeof(int));
static_assert(sizeof(remove_extent<long[]>::type) == 8);
