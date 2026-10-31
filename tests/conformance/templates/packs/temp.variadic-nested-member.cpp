// [temp.variadic]: packs of an enclosing template in a member template.
template <class... Ts> struct outer {
  template <class... Us> struct inner { static constexpr int v = sizeof...(Ts) * 10 + sizeof...(Us); };
};
static_assert(outer<int, int>::inner<char>::v == 21);
static_assert(outer<>::inner<>::v == 0);
