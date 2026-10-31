// [temp.variadic]/5: a function parameter pack expanded in a call.
template <class... Ts> constexpr int sum(Ts... ts) { return (ts + ... + 0); }
template <class... Ts> constexpr int fwd(Ts... ts) { return sum(ts...); }
template <class... Ts> constexpr int twice(Ts... ts) { return sum((ts * 2)...); }
template <class... Ts> constexpr int both(Ts... ts) { return sum(ts..., ts...); }
static_assert(fwd(1, 2, 3, 4) == 10);
static_assert(twice(1, 2, 3) == 12);
static_assert(both(1, 2) == 6);
