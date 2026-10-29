// [temp.spec.partial]/8, [temp.deduce.type]: partial specializations of a
// variable template on non-type arguments.
template <int N, int M> constexpr int cmp = N < M ? -1 : 1;
template <int N> constexpr int cmp<N, N> = 0;
static_assert(cmp<1, 2> == -1);
static_assert(cmp<3, 2> == 1);
static_assert(cmp<4, 4> == 0);
template <class T, unsigned long N> constexpr unsigned long count = 0;
template <class T, unsigned long N> constexpr unsigned long count<T[N], N> = N;
static_assert(count<int[5], 5> == 5);
static_assert(count<int[5], 4> == 0);
