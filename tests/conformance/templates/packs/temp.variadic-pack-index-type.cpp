// [dcl.type.pack.index] (C++26, P2662): `T...[N]` selects the Nth type of
// the pack, with a constant or template-parameter index.
template <class... Ts> using first = Ts...[0];
template <int I, class... Ts> using at = Ts...[I];
template <class... Ts> using last = Ts...[sizeof...(Ts) - 1];
static_assert(__is_same(first<int, char>, int));
static_assert(__is_same(at<1, int, char, long>, char));
static_assert(__is_same(at<2, int, char, long>, long));
static_assert(__is_same(last<int, char, long>, long));
static_assert(__is_same(last<void>, void));
