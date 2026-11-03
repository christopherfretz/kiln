// [temp.variadic]/8, [expr.sizeof]/5: `sizeof...` of an alias template's
// pack bound to a pack expansion stays value-dependent until that pack is
// substituted, so a template-id it selects is not formed early.
template<__SIZE_TYPE__ N> struct Int { static constexpr __SIZE_TYPE__ value = N; };
template<class... A> using Count = Int<sizeof...(A)>;
template<class... T> struct S {
  using C0 = Count<T...>;
  using C1 = Count<int, T...>;
  using C2 = Count<T..., T*...>;
};
static_assert(S<>::C0::value == 0);
static_assert(S<>::C1::value == 1);
static_assert(S<char, char>::C1::value == 3);
static_assert(S<char, char>::C2::value == 4);

struct false_type { static constexpr bool value = false; };
struct true_type { static constexpr bool value = true; };
template<bool> struct OrImpl;
template<> struct OrImpl<true> {
  template<class R, class First, class... Rest>
  using Result = typename OrImpl<!bool(First::value) && sizeof...(Rest) != 0>::template Result<First, Rest...>;
};
template<> struct OrImpl<false> {
  template<class R, class...> using Result = R;
};
template<class... A>
using Or = typename OrImpl<sizeof...(A) != 0>::template Result<false_type, A...>;
template<class... T> struct D : Or<T...> {};
static_assert(!D<>::value);
static_assert(!D<false_type, false_type>::value);
static_assert(D<false_type, true_type>::value);
