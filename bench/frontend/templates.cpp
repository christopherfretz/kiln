//@ args: -stdlib=libc++
// Template-heavy front-end work with almost no constant evaluation:
// thousands of class template instantiations, variadic packs, recursive
// type lists, concepts and overload resolution. Measures sema, not the Oven.
#include <concepts>
#include <cstddef>
#include <tuple>
#include <type_traits>
#include <utility>

template <class... Ts> struct type_list {};

template <std::size_t I> struct tag { static constexpr std::size_t value = I; };

// Recursive (non-pack-indexing) list operations: one instantiation per step.
template <class L, class T> struct push_back;
template <class... Ts, class T> struct push_back<type_list<Ts...>, T> {
  using type = type_list<Ts..., T>;
};

template <std::size_t N, class Acc = type_list<>> struct make_tags {
  using type = typename make_tags<N - 1, typename push_back<Acc, tag<N - 1>>::type>::type;
};
template <class Acc> struct make_tags<0, Acc> { using type = Acc; };

template <class L> struct reverse;
template <> struct reverse<type_list<>> { using type = type_list<>; };
template <class T, class... Ts> struct reverse<type_list<T, Ts...>> {
  using type = typename push_back<typename reverse<type_list<Ts...>>::type, T>::type;
};

template <template <class> class F, class L> struct filter;
template <template <class> class F> struct filter<F, type_list<>> { using type = type_list<>; };
template <template <class> class F, class T, class... Ts> struct filter<F, type_list<T, Ts...>> {
  using rest = typename filter<F, type_list<Ts...>>::type;
  template <class L> struct cons;
  template <class... Us> struct cons<type_list<Us...>> { using type = type_list<T, Us...>; };
  using type = std::conditional_t<F<T>::value, typename cons<rest>::type, rest>;
};

template <class T> struct is_even : std::bool_constant<T::value % 2 == 0> {};

template <class L> struct size;
template <class... Ts> struct size<type_list<Ts...>> {
  static constexpr std::size_t value = sizeof...(Ts);
};

template <std::size_t N> struct pipeline {
  using tags = typename make_tags<N>::type;
  using rev = typename reverse<tags>::type;
  using evens = typename filter<is_even, rev>::type;
  static_assert(size<evens>::value == (N + 1) / 2);
};

template struct pipeline<200>;
template struct pipeline<190>;
template struct pipeline<180>;
template struct pipeline<170>;

// Concepts and overload resolution over many distinct types.
template <class T> concept has_value = requires { T::value; };
template <class T> concept small_tag = has_value<T> && (T::value < 64);

template <class T> constexpr int classify(T) { return 0; }
template <has_value T> constexpr int classify(T) { return 1; }
template <small_tag T> constexpr int classify(T) { return 2; }

template <std::size_t... Is> constexpr int classify_all(std::index_sequence<Is...>) {
  return (classify(tag<Is>{}) + ...);
}
static_assert(classify_all(std::make_index_sequence<250>{}) == 64 * 2 + (250 - 64));

// Large std::tuple instantiations with std::get and std::apply.
template <std::size_t... Is> constexpr std::size_t tuple_sum(std::index_sequence<Is...>) {
  std::tuple<tag<Is>...> t{};
  return std::apply([](auto... xs) { return (decltype(xs)::value + ... + 0); }, t) +
         std::get<sizeof...(Is) - 1>(t).value;
}
static_assert(tuple_sum(std::make_index_sequence<120>{}) == 119 * 120 / 2 + 119);
