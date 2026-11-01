// GNU `__extension__` before a template-declaration and an explicit
// specialization, at namespace and class scope: libstdc++ <type_traits>:788
// (`__extension__ template<typename _Tp> using __is_signed_integer = ...`),
// ext/numeric_traits.h:130 and <limits>:1658 (the `__int128` traits, emitted
// under -std=c++NN because it defines __STRICT_ANSI__). It was the first error
// of 79 of the 109 libstdc++ headers until WP-237; this keeps the alias,
// variable and member template forms covered (docs/status/libstdcxx.md,
// WP-188). Verified against clang 19.
//@ expect: compile-ok
template<typename T> struct is_int { static constexpr bool value = false; };
__extension__ template<> struct is_int<__int128> { static constexpr bool value = true; };
__extension__ template<typename T> using id = T;
__extension__ __extension__ template<typename T> constexpr T zero = T();
struct S {
  __extension__ template<typename T> static constexpr T one() { return T(1); }
};
static_assert(is_int<__int128>::value && !is_int<int>::value);
static_assert(sizeof(id<int>) == sizeof(int));
static_assert(zero<long> == 0 && S::one<int>() == 1);
