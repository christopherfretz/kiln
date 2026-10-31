// [temp.arg.explicit]/6: with T explicitly specified, the arguments convert.
template <class T> constexpr T mx(T a, T b) { return a < b ? b : a; }
static_assert(mx<double>(1, 2.5) == 2.5);
static_assert(mx<int>(3, 2.5) == 3);
