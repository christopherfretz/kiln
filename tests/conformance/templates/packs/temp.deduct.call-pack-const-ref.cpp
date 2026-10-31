// [temp.deduct.call]/2: a const T& pattern strips top-level cv from each A.
template <class... Ts> constexpr bool none_const(const Ts &...) { return (!__is_const(Ts) && ...); }
const int c = 1;
static_assert(none_const(c, 2, 'x'));
