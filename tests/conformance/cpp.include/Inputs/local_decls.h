// WP-1320: block-scope static_assert declarations, typedefs and alias
// declarations in a header's function bodies (the shapes of `declval` and
// `__call_is_nt` in <type_traits>).
#ifndef LOCAL_DECLS_H
#define LOCAL_DECLS_H
namespace lcd {
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
template <class T> constexpr bool small() {
  using P = T *;
  static_assert(sizeof(P) >= sizeof(char), "pointer");
  return sizeof(T) < sizeof(P);
}
inline int plain() {
  using I = int;
  typedef I J;
  static_assert(sizeof(I) == sizeof(J), "same");
  J x = 3;
  return x;
}
} // namespace lcd
#endif
