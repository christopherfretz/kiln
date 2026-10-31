// [temp.deduct.type]/9: a non-type pack deduced from a template-id.
template <int... Ns> struct il {};
template <int... Ns> constexpr int sum(il<Ns...>) { return (Ns + ... + 0); }
static_assert(sum(il<1, 2, 3>{}) == 6);
static_assert(sum(il<>{}) == 0);
