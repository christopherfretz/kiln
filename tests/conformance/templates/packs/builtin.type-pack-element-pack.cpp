// __type_pack_element consumes an expanded pack.
template <class... Ts> using second = __type_pack_element<1, Ts...>;
static_assert(__is_same(second<int, long, char>, long));
template <unsigned long I, class... Ts> using at = __type_pack_element<I, Ts...>;
static_assert(__is_same(at<0, char>, char));
static_assert(__is_same(at<2, int, int, double>, double));
