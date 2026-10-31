// [temp.spec.partial]: recursive type-list reversal through partial specializations.
template <class... Ts> struct tl {};
template <class In, class Out = tl<>> struct rev;
template <class... Out> struct rev<tl<>, tl<Out...>> { using type = tl<Out...>; };
template <class T, class... Ts, class... Out> struct rev<tl<T, Ts...>, tl<Out...>> {
  using type = typename rev<tl<Ts...>, tl<T, Out...>>::type;
};
static_assert(__is_same(rev<tl<int, char, long>>::type, tl<long, char, int>));
