// [temp.constr.normal]/1.4 (P2841R7), [temp.constr.fold], spec/sema-templates.md
// §8.2 "Concept template parameters": a higher-order concept
// `all_of<C, Ts...>` checked over 10,000 distinct types, for two argument
// concepts, then the same checks again through a second concept-id and a
// constrained function template. Satisfaction is memoised on the
// substituted concept-id (the concept argument is part of the key), so the
// repeats cost a lookup and the 10,000 `C<T>` atoms are evaluated once per
// argument concept (debug build: ~1.2 s and ~36 MB per 10,000-type check,
// the repeats free). WP-746.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ skip(clang): clang 22 reports all_of<Small, T<0>..T<N-1>> unsatisfied once N reaches about 2500
//@ mem<=1024
template <int N> struct T { static constexpr int v = N; };
template <class X> concept Small = X::v < 20000;
template <class X> concept NonNeg = X::v >= 0;
template <class X> concept Odd = X::v % 2 == 1;
template <template <class> concept C, class... Ts> concept all_of = (C<Ts> && ...);
template <template <class> concept C, class... Ts> concept any_of = (C<Ts> || ...);

template <class... Ts> struct list {};
template <class I, I... Is> struct seq {};
template <class L> struct check;
template <class... Ts> struct check<list<Ts...>> {
  static constexpr bool small = all_of<Small, Ts...>;
  static constexpr bool nonneg = all_of<NonNeg, Ts...>;
  static constexpr bool odd = all_of<Odd, Ts...>;
  static constexpr bool some_odd = any_of<Odd, Ts...>;
  template <template <class> concept C> requires all_of<C, Ts...>
  static constexpr int f() { return 1; }
  template <template <class> concept C> static constexpr int f() { return 2; }
};
template <int... Is> using L = list<T<Is>...>;
template <class S> struct to_list;
template <int... Is> struct to_list<seq<int, Is...>> { using type = L<Is...>; };
using C10k = check<to_list<__make_integer_seq<seq, int, 10000>>::type>;
static_assert(C10k::small && C10k::nonneg && !C10k::odd && C10k::some_odd);
// The same concept-ids again: memoised.
static_assert(C10k::f<Small>() == 1 && C10k::f<NonNeg>() == 1 && C10k::f<Odd>() == 2);
static_assert(C10k::f<Small>() == 1 && C10k::f<NonNeg>() == 1);
