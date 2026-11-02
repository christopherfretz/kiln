// [temp.spec.partial.order]/1, [temp.deduct.type]/9: during partial
// ordering a pack expansion in the synthesized arguments is deduced as a
// whole by a pack expansion of the other partial specialization, whatever
// the index of the pack parameters (libc++ `__make_dextents`).
template <class I, unsigned long... E> struct ext {};
template <class... T> struct tl {};

template <class I, unsigned long R, class X = ext<I>> struct mk;
template <class I, unsigned long R, unsigned long... P>
struct mk<I, R, ext<I, P...>> {
  using type = typename mk<I, R - 1, ext<I, ~0UL, P...>>::type;
};
template <class I, unsigned long... P> struct mk<I, 0, ext<I, P...>> {
  using type = ext<I, P...>;
};

template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
static_assert(same<mk<long, 0>::type, ext<long>>::value);
static_assert(same<mk<long, 2>::type, ext<long, ~0UL, ~0UL>>::value);

// A type parameter pack, and the more specialized one declared first.
template <int R, class X> struct pick { static constexpr int v = 0; };
template <class... P> struct pick<0, tl<P...>> { static constexpr int v = 2; };
template <int R, class... P> struct pick<R, tl<P...>> { static constexpr int v = 1; };
static_assert(pick<0, tl<int, char>>::v == 2);
static_assert(pick<0, tl<>>::v == 2);
static_assert(pick<3, tl<int>>::v == 1);
static_assert(pick<3, int>::v == 0);
