// [dcl.inline]/1, [temp.pre]/1: an inline variable template; its
// specializations are inline variables.
template <class T> inline constexpr bool always = true;
template <class T> inline constexpr T max_of = T(~0u >> 1);
static_assert(always<int> && always<void>);
static_assert(max_of<int> == 2147483647);
inline constexpr int k = max_of<int> / 2;
static_assert(k == 1073741823);
