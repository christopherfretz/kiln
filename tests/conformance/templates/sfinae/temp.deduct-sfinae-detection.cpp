// [temp.deduct]/8: detection of a member type with a two-overload test.
template <class T> struct has_type {
  template <class U> static constexpr bool test(typename U::type*) { return true; }
  template <class U> static constexpr bool test(...) { return false; }
  static constexpr bool value = test<T>(nullptr);
};
struct A { using type = int; };
struct B {};
static_assert(has_type<A>::value);
static_assert(!has_type<B>::value);
static_assert(!has_type<int>::value);
