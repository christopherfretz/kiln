// [dcl.fct.spec]/4: the constant-expression of an explicit-specifier is
// contextually converted to bool, so a class-type prvalue with a constexpr
// conversion operator is valid. libstdc++ bits/stl_pair.h:344
// `explicit(__not_<__and_<...>>())`; <functional>, <unordered_map>, 29 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
template<bool B> struct bool_constant {
  static constexpr bool value = B;
  constexpr operator bool() const noexcept { return B; }
};
template<class T> struct is_small : bool_constant<(sizeof(T) < 4)> {};
template<class C> struct not_ : bool_constant<!bool(C::value)> {};
template<class T> struct box {
  T t;
  constexpr explicit(not_<is_small<T>>()) box(T v) : t(v) {}
  constexpr explicit(not_<is_small<T>>()) box() : t() {}
};
constexpr box<char> a = 'x';
constexpr box<long> b(1L);
static_assert(a.t == 'x' && b.t == 1);
template<class T> constexpr bool implicit_from = requires(void (*f)(box<T>)) { f(T()); };
static_assert(implicit_from<char> && !implicit_from<long>);
constexpr box<char> c = {};
