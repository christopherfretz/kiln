// [temp.variadic]/5, [temp.alias]/2: an alias template whose parameter
// pack is bound to arguments that are themselves packs of the enclosing
// template (`ct_t<T, U>...` with `template <class... A> using ct_t`):
// each element of the alias's pack is a pack of the enclosing template,
// expanded by the outer `...` (libc++ <tuple> `common_type<tuple<...>>`).
template <class... T> struct tup {};
template <class... A> struct ct;
template <class A, class B> struct ct<A, B> { using type = B; };
template <class... A> using ct_t = typename ct<A...>::type;
template <class A, class B> struct cr;
template <class... T, class... U>
  requires requires { typename tup<ct_t<T, U>...>; }
struct cr<tup<T...>, tup<U...>> {
  using type = tup<ct_t<T, U>...>;
};
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
static_assert(same<cr<tup<int, char>, tup<long, short>>::type, tup<long, short>>::value);
static_assert(same<cr<tup<>, tup<>>::type, tup<>>::value);
