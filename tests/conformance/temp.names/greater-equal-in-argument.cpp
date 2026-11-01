// [temp.names]/4: only `>` and the first `>` of `>>` end a template
// argument list. `>=` inside an argument is a relational operator, so
// `BC<sizeof...(T) >= 1>` is one argument and `BC<...>::value` a
// qualified name (libc++ <tuple>:575).
template <bool B> struct BC { static constexpr bool value = B; };
template <class... T> struct S {
  static constexpr bool a = BC<sizeof...(T) >= 1>::value;
  static constexpr bool b = BC<(sizeof...(T) >= 2)>::value;
};
static_assert(S<int>::a && !S<>::a);
static_assert(S<int, int>::b && !S<int>::b);

template <int N> struct I { static constexpr int v = N; };
constexpr int k = 3;
static_assert(BC<k >= 2>::value);
static_assert(!BC<k >= 4>::value);
static_assert(I<(k >= 2)>::v == 1);
static_assert(I<(k >> 1)>::v == 1);

template <class... T> using E = BC<sizeof...(T) >= 1>;
static_assert(E<int>::value && !E<>::value);

template <class T> struct X {};
X<X<int>>* p;
template <bool B, class T = int> struct EnableIf {};
template <class T> struct EnableIf<true, T> { using type = T; };
template <class... T, typename EnableIf<BC<sizeof...(T) >= 1>::value, int>::type = 0>
constexpr int f(T...) { return 1; }
static_assert(f(1, 2) == 1);
