// [temp.deduct.type]/9: two packs deduced from two arguments.
template <class... Ts> struct tl {};
template <class... A, class... B> constexpr int two(tl<A...>, tl<B...>) { return sizeof...(A) * 10 + sizeof...(B); }
static_assert(two(tl<int>{}, tl<int, int>{}) == 12);
static_assert(two(tl<>{}, tl<>{}) == 0);
