// [temp.variadic]/7, [temp.deduct.general]/8: every pack named in a pack
// expansion must have the same length, and a mismatch found while
// substituting into a default template argument or a return type is a
// deduction failure. Here the first pack the expansion names is empty and a
// later one is not: `all<is_convertible(U, T)...>` with `T` empty and `U`
// = {int}. Boost.Fusion `tuple<>` (is_constructible<tuple<>, int>),
// Boost.HOF `pack_base` (hof/pack.hpp:252) and Boost.Hana integer_sequence
// `equal` hit this.
//@ expect: run
//@ stdout: 0 1 0 1 0
extern "C" int printf(const char*, ...);
template<bool... B> struct all { static constexpr bool value = (B && ...); };
template<bool C, class T = void> struct enable_if {};
template<class T> struct enable_if<true, T> { using type = T; };

template<class... T> struct tup {
  template<class... U, class = typename enable_if<all<__is_convertible_to(U, T)...>::value>::type>
  explicit tup(U&&...) {}
  tup() {}
};

template<class... Ts> struct pk {
  template<class... Xs, class = typename enable_if<all<__is_convertible_to(Xs, Ts)...>::value>::type>
  pk(Xs&&...) {}
};

template<class T, T... v> struct seq {};
template<bool B> struct b_ { static constexpr bool value = B; };
struct E {
  template<class X, X... xs, class Y, Y... ys>
  static constexpr b_<all<(xs == ys)...>::value> apply(seq<X, xs...>, seq<Y, ys...>) { return {}; }
  template<class Xs, class Ys>
  static constexpr b_<false> apply(Xs, Ys, ...) { return {}; }
};

int main() {
  printf("%d %d %d %d %d\n",
         (int)__is_constructible(tup<>, int),
         (int)__is_constructible(tup<long>, int),
         (int)__is_constructible(pk<>, int, int),
         (int)decltype(E::apply(seq<int>{}, seq<long>{}))::value,
         (int)decltype(E::apply(seq<int>{}, seq<long, 0>{}))::value);
}
