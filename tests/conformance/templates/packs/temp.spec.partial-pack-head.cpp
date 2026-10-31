// [temp.spec.partial.match]: X<T, Ts...> peels the first element.
template <class... Ts> struct tl {};
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
template <class T> struct head;
template <class T, class... Ts> struct head<tl<T, Ts...>> {
  using type = T;
  using rest = tl<Ts...>;
};
using H = head<tl<int, char, long>>;
static_assert(same<typename H::type, int>);
static_assert(same<typename H::rest, tl<char, long>>);
static_assert(same<typename head<tl<int>>::rest, tl<>>);
