// [temp.deduce.type]/8: the bound of an array type is deduced in a
// partial specialization of a variable template.
template <class T> constexpr unsigned long extent = 0;
template <class T, unsigned long N> constexpr unsigned long extent<T[N]> = N;
template <class T> constexpr unsigned long rank = 0;
template <class T> constexpr unsigned long rank<T[]> = rank<T> + 1;
template <class T, unsigned long N> constexpr unsigned long rank<T[N]> = rank<T> + 1;
static_assert(extent<int> == 0);
static_assert(extent<int[7]> == 7);
static_assert(extent<int[2][3]> == 2);
static_assert(rank<int> == 0);
static_assert(rank<int[2][3][4]> == 3);
static_assert(rank<int[][2]> == 2);
