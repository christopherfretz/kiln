// [temp.deduct.type]/9: a pack deduced from a class template-id's argument list.
template <class... Ts> struct tl {};
template <class... Ts> constexpr unsigned long len(tl<Ts...>) { return sizeof...(Ts); }
static_assert(len(tl<int, char, long>{}) == 3);
static_assert(len(tl<>{}) == 0);
template <class T, class... Ts> constexpr unsigned long rest(tl<T, Ts...>) { return sizeof...(Ts); }
static_assert(rest(tl<int, char>{}) == 1);
