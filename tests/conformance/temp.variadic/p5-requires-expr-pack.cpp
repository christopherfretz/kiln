// [temp.variadic]/5, [expr.prim.req.general]: a requires-expression
// contains an unexpanded pack when its parameter-declaration-clause or
// its requirements do, so it can be the pattern of a fold or of a
// template-argument pack expansion; each element instantiates it with
// fresh local parameters.
//@ expect: compile-ok
struct N {};
struct HT { using type = int; };
template <class T> concept C = requires { requires true; };

// Unary folds over && and ||.
template<class... Ts>
constexpr bool all_lt = (requires(const Ts& t) { t < t; } && ...);
template<class... Ts>
constexpr bool any_lt = (requires(const Ts& t) { t < t; } || ...);
static_assert(all_lt<int, long>);
static_assert(!all_lt<int, N>);
static_assert(any_lt<N, int>);
static_assert(!any_lt<N>);
static_assert(all_lt<>);
static_assert(!any_lt<>);

// Binary folds.
template<class... Ts>
constexpr bool bin_and = (true && ... && requires(Ts t) { t + t; });
template<class... Ts>
constexpr bool bin_or = (requires(Ts t) { t + t; } || ... || false);
static_assert(bin_and<int, double>);
static_assert(!bin_and<int, N>);
static_assert(bin_or<N, int>);
static_assert(!bin_or<N, N>);
static_assert(bin_and<>);
static_assert(!bin_or<>);

// A template-argument pack expansion; two pack-typed parameters.
template<bool... B> struct all { static constexpr bool value = (B && ...); };
template<class... Ts>
constexpr bool targ = all<requires(const Ts& t, Ts* p) { *p = t; }...>::value;
static_assert(targ<int, double>);
static_assert(!targ<int, const N>);

// Packs of two levels, in one requires-expression and nested.
template<class... Ts> struct outer {
  template<class... Us>
  static constexpr bool inner = (requires(Ts t, Us u) { t = u; } && ...);
};
static_assert(outer<int, long>::inner<short, char>);
static_assert(!outer<N, int>::inner<int, int>);
template<class... Ts> struct nest {
  template<class... Us>
  static constexpr bool all = ((requires(Ts t) { (requires(Us u) { t = u; } && ...); }) && ...);
};
static_assert(nest<int, long>::all<short, char>);

// sizeof... inside; a type requirement; a compound requirement.
template<class... Ts>
constexpr bool sz = (requires(Ts t) { requires sizeof...(Ts) > 1; } && ...);
static_assert(sz<int,int>);
static_assert(!sz<int>);
template<class... Ts>
constexpr bool typ = (requires { typename Ts::type; } && ...);
static_assert(typ<HT, HT>);
static_assert(!typ<HT, int>);
template<class... Ts>
constexpr bool cmp = (requires(const Ts& t) { { t < t } -> C; } && ...);
static_assert(cmp<int, long>);
static_assert(!cmp<int, N>);

// An expanded parameter pack of a requires-expression is not an
// unexpanded pack of the expression.
template <class... Ts>
constexpr bool local = requires(const Ts&... t) { (t, ...); };
static_assert(local<int, N>);

// A requirement that fails at the enclosing template's instantiation
// settles the value even though a local parameter still depends on the
// generic lambda's own (retained) level.
template <class... Ts> constexpr auto gen() {
  return [](auto u) { return requires(Ts... t, decltype(u) v) { (t + ... + 0); }; };
}
static_assert(gen<int, long>()(N{}));
static_assert(!gen<N, int>()(0));
