export module imglcd;
export namespace lcd {
template <class T> struct id {
  using type = T;
};
template <class T> struct never {
  static constexpr bool value = false;
};
template <class T> T &&odr() noexcept {
  static_assert(never<T>::value, "odr must not be odr-used");
  return static_cast<T &&>(*static_cast<T *>(nullptr));
}
template <class T> constexpr int sizes() {
  using U = typename id<T>::type;
  typedef U V;
  static_assert(sizeof(V) > 0);
  return int(sizeof(U) + sizeof(V));
}
inline int plain() {
  using I = int;
  typedef I J;
  static_assert(sizeof(I) == sizeof(J), "same");
  J x = 3;
  return x;
}
template <class T> constexpr bool nothrow_odr = noexcept(odr<T>());
} // namespace lcd
