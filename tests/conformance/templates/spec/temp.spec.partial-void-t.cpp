// [temp.spec.partial.match]/2, [temp.deduce]/8 (SFINAE): the detection
// idiom; a partial specialization whose arguments fail to substitute does
// not match.
template <class...> using void_t = void;
template <class T, class = void> struct has_type { static constexpr bool value = false; };
template <class T> struct has_type<T, void_t<typename T::type>> {
  static constexpr bool value = true;
};
struct A { using type = int; };
struct B {};
static_assert(has_type<A>::value);
static_assert(!has_type<B>::value);
static_assert(!has_type<int>::value);
