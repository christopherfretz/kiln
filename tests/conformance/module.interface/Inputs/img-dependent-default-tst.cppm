export module imgddt;
export namespace ddt {
template <bool B> struct bool_constant {
  static constexpr bool value = B;
  typedef bool_constant type;
};
template <typename T> struct is_arith : bool_constant<false> {};
template <> struct is_arith<int> : bool_constant<true> {};
template <typename T, bool = is_arith<T>::value> struct helper
    : bool_constant<false> {};
template <typename T> struct helper<T, true> : bool_constant<T(-1) < T(0)> {};
template <typename T> struct is_signed : public helper<T>::type {};
template <typename T, int N = sizeof(T)> struct sized {
  typedef char type[N];
};
template <typename T> struct holder {
  typename sized<T>::type bytes;
};
} // namespace ddt
