// GNU `__extension__` before a template declaration at namespace and class
// scope (WP-237; libstdc++ <ext/numeric_traits.h> `_GLIBCXX_INT_N_TRAITS`
// and <type_traits> `__extension__ template<typename _Tp> using
// __is_signed_integer = ...`, reached from <complex>): the keyword is
// dropped and the declaration parsed as usual ([temp.pre]; clang
// `ParseExternalDeclaration`). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class T> struct is_int { static constexpr bool value = false; };
__extension__ template <> struct is_int<__int128> {
  static constexpr bool value = true;
};
__extension__ template <class T> using id = T;
__extension__ __extension__ template <class T> constexpr int zero = 0;
struct S {
  __extension__ template <class T> static constexpr T get(T t) { return t; }
};
static_assert(is_int<id<__int128>>::value && zero<int> == 0);
static_assert(S::get(3) == 3);
