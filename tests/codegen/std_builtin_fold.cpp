// std::move, forward, addressof, as_const and their kin fold (WP-172):
// clang treats them as builtins at -O0, so a call is the address of the
// argument and the function is never emitted.
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
template <class T> constexpr T* addressof(T& t) noexcept { return __builtin_addressof(t); }
template <class T> const T* addressof(const T&&) = delete;
template <class T> constexpr T&& move_if_noexcept(T& t) noexcept { return static_cast<T&&>(t); }
// The algorithm std::move is not the builtin.
template <class I, class O> O move(I f, I l, O o) { for (; f != l; ++f, ++o) *o = static_cast<decltype(*f)&&>(*f); return o; }
}
}
namespace other {
template <class T> T&& move(T& t) { return static_cast<T&&>(t); }
}
struct S { int a, b; S() = default; S(S&& o) : a(o.a), b(o.b) {} void operator&() const; };
struct E {};
void take(S&&);
void take2(const S&);
template <class T> void fwd(T&& t) { take(std::forward<T>(t)); }
int use(S& s, int* p, int* q) {
  S t(std::move(s));
  take(std::move(t));
  take2(std::as_const(t));
  S* a = std::addressof(t);
  const S* b = __builtin_addressof(t);
  fwd(std::move(t));
  int x = std::move(*p);
  int&& r = std::move_if_noexcept(x);
  std::move(x);
  std::move(p, p + 2, q);
  E e;
  E e2 = std::move(e);
  int y = other::move(x);
  return a->a + b->b + r + y + std::move(S()).a;
}
