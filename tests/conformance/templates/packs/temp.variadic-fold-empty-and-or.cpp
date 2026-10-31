// [temp.variadic]/10: an empty unary fold of && is true, of || is false.
template <bool... Bs> constexpr bool all = (Bs && ...);
template <bool... Bs> constexpr bool any = (... || Bs);
static_assert(all<>);
static_assert(!any<>);
static_assert(all<true, true>);
static_assert(!all<true, false>);
static_assert(any<false, true>);
