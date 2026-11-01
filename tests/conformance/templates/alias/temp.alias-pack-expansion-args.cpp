// [temp.alias]/2, [temp.variadic]/5: an alias template named with a pack
// expansion that lands on its trailing parameter pack is substituted
// right away, so the argument types stay deducible ([temp.deduct.type]);
// a pack element that is itself an expansion is substituted as its
// pattern and re-expanded (libc++ `__index_sequence`).
template <class T, T... I> struct iseq {};
template <int... I> using idx = iseq<int, I...>;
template <int... Is> constexpr int g(idx<Is...>) { return sizeof...(Is); }
static_assert(g(iseq<int, 0, 1, 2>{}) == 3);

template <class T, T v> struct ic { static constexpr T value = v; };
template <class... U> using N = ic<int, sizeof...(U)>;
template <class... Ts> constexpr int f() { return N<Ts...>::value; }
static_assert(f<int, char, long>() == 3);

template <class... U> struct tl {};
template <class... U> using T2 = tl<U *...>;
template <class... Ts> constexpr int g2(T2<Ts...>) { return sizeof...(Ts); }
static_assert(g2(tl<int *, char *>{}) == 2);
template <class A, class... U> using T3 = tl<A, U...>;
template <class... Ts> constexpr int h(T3<int, Ts...>) { return sizeof...(Ts); }
static_assert(h(tl<int, char, long>{}) == 2);
template <int... I> using T4 = tl<ic<int, I>...>;
template <int... Is> constexpr int k(T4<Is...>) { return sizeof...(Is); }
static_assert(k(tl<ic<int, 1>, ic<int, 2>>{}) == 2);
template <class... U> using T5 = tl<tl<U...>, U...>;
template <class... Ts> constexpr int m(T5<Ts...>) { return sizeof...(Ts); }
static_assert(m(tl<tl<int, char>, int, char>{}) == 2);

template <class... Ts> struct W { using X = T2<Ts...>; };
static_assert(__is_same(W<int, char>::X, tl<int *, char *>));
template <int... Is> struct W4 { using X = T4<Is...>; };
static_assert(__is_same(W4<1, 2>::X, tl<ic<int, 1>, ic<int, 2>>));
template <class... Ts> struct W5 { using X = T5<Ts...>; };
static_assert(__is_same(W5<int, char>::X, tl<tl<int, char>, int, char>));
