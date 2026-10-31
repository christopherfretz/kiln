// [temp.spec.partial.match]: a variadic partial specialization X<Ts...>.
template <class... Ts> struct tl {};
template <class T> struct len;
template <class... Ts> struct len<tl<Ts...>> { static constexpr int value = sizeof...(Ts); };
static_assert(len<tl<int, char>>::value == 2);
static_assert(len<tl<>>::value == 0);
