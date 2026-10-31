// [temp.spec.partial.match]: partial specializations of a variadic primary template.
template <class... Ts> struct count { static constexpr int v = 0; };
template <class T, class... Ts> struct count<T, Ts...> { static constexpr int v = 1 + count<Ts...>::v; };
static_assert(count<>::v == 0);
static_assert(count<int, char, long>::v == 3);
