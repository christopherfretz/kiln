// [temp.variadic]/8: two packs of equal length expanded in one pattern.
template <class... Ts> struct tl {};
template <class... A, class... B> constexpr unsigned long pairs(tl<A...>, tl<B...>) {
  return (0 + ... + (sizeof(A) * sizeof(B)));
}
static_assert(pairs(tl<char, short>{}, tl<int, char>{}) == sizeof(int) + sizeof(short));
