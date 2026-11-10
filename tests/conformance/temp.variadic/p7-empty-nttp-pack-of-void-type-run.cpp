// [temp.variadic]/7, [temp.param]/4: an expansion of an empty pack
// instantiates nothing, so its pattern is never formed. With
// `template<class T, T... v>` and `T` = void, the pattern
// `ic<T, v>...` would name a non-type parameter of type void, which is
// ill-formed ([temp.param]/4), but `v` is empty and no `ic<void, ...>`
// is formed: a variable template, a member typedef and an alias
// template all accept `<void>`, and in a function parameter type it is
// no deduction failure ([temp.deduct.general]/8). Boost.Hana
// `tuple_c<void>` (hana/fwd/tuple.hpp:225, test/tuple/special.empty.cpp)
// hits this.
//@ expect: run
//@ stdout: 0 0 0 0 2 2
extern "C" int printf(const char*, ...);
template <typename T, T v> struct ic { static constexpr T value = v; };
template <typename... X> struct tup { static constexpr int size = sizeof...(X); };
template <typename T, T... v> struct seq {};

template <typename T, T... v> constexpr tup<ic<T, v>...> tuple_c{};
template <typename T, T... v> struct C { using type = tup<ic<T, v>...>; };
template <typename T, T... v> using A = tup<ic<T, v>...>;
template <typename T, T... v> int g(seq<T, v...>, tup<ic<T, v>...>* = nullptr) {
  return sizeof...(v);
}

int main() {
  constexpr auto t = tuple_c<void>;
  static_assert(decltype(t)::size == 0);
  static_assert(C<void>::type::size == 0);
  static_assert(A<void>::size == 0);
  constexpr auto u = tuple_c<int, 1, 2>;
  printf("%d %d %d %d %d %d\n", decltype(t)::size, C<void>::type::size, A<void>::size,
         g(seq<void>{}), decltype(u)::size, g(seq<int, 1, 2>{}));
  return 0;
}
