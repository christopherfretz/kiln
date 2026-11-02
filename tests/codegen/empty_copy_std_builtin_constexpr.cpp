// [class.copy.ctor]/14, [expr.const]/5: a trivial copy of an empty class
// from `std::move(ce)` (or forward/as_const/move_if_noexcept) of a
// constexpr `ce` has a constant initializer, as clang's
// isConstantInitializer evaluates through the std builtins; it is
// emitted as a memcpy from a `@__const` global. From a non-constexpr
// source it is emitted as written (WP-276).
namespace std {
inline namespace __1 {
template <class T> struct remove_reference { using type = T; };
template <class T> struct remove_reference<T&> { using type = T; };
template <class T> struct remove_reference<T&&> { using type = T; };
template <class T> constexpr typename remove_reference<T>::type&& move(T&& t) noexcept {
  return static_cast<typename remove_reference<T>::type&&>(t);
}
template <class T> constexpr T&& forward(typename remove_reference<T>::type& t) noexcept {
  return static_cast<T&&>(t);
}
template <class T> constexpr const T& as_const(T& t) noexcept { return t; }
template <class T> constexpr T&& move_if_noexcept(T& t) noexcept { return static_cast<T&&>(t); }
}
}
struct E {};
void sink(E);
int main() {
  constexpr E ce;
  E e;
  E e1 = std::move(ce);
  E e2 = (std::move(ce));
  E e3 = std::forward<const E&>(ce);
  E e4 = std::as_const(ce);
  E e5 = std::move(std::as_const(ce));
  E e6 = std::move(e);
  E e7 = std::as_const(e);
  E e8 = std::move_if_noexcept(ce);
  sink(e1); sink(e2); sink(e3); sink(e4); sink(e5); sink(e6); sink(e7); sink(e8);
}
