// [expr.prim.pack.index] (C++26, P2662): `p...[N]` selects the Nth element
// of a function parameter pack; its type is that element's.
template <class... Ts> constexpr auto second(Ts... ts) { return ts...[1]; }
template <class... Ts> constexpr auto last(Ts... ts) {
  return ts...[sizeof...(ts) - 1];
}
static_assert(second(1, 2L, 3) == 2L);
static_assert(__is_same(decltype(second(1, 2L)), long));
static_assert(last(1, 2, 3, 40) == 40);
static_assert(last('a') == 'a');
