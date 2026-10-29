// [temp.param]/4, [temp.arg.nontype]: variable templates with non-type
// template parameters.
template <int N> constexpr int square = N * N;
template <int N, int M> constexpr int sum = N + M;
template <bool B> constexpr int pick = B ? 1 : 2;
static_assert(square<7> == 49);
static_assert(square<-3> == 9);
static_assert(sum<2, 3> == 5);
static_assert(pick<true> == 1 && pick<false> == 2);
static_assert(square<square<2>> == 16);
