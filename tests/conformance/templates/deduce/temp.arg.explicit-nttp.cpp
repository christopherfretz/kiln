// [temp.arg.explicit]: a non-type template parameter given explicitly.
template <int N, class T> constexpr T scale(T x) { return x * N; }
static_assert(scale<3>(4) == 12);
static_assert(scale<2>(1.5) == 3.0);
