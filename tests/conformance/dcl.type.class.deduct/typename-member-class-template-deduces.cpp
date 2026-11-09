// [dcl.type.class.deduct]/1, [temp.res.general]/4: a typename-specifier
// naming a member class template is a placeholder for a deduced class type
// as a variable's declared type, dependent or not; the deduced type is the
// specialization for the initializer's type. WP-958.
//@ std: c++20
//@ expect: compile-ok
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
struct A {
  template <typename T> struct X {
    constexpr X(T) {}
  };
};
template <typename T> constexpr bool g(T t) {
  typename T::X x = t;
  return same<decltype(x), typename T::template X<T>>;
}
static_assert(g(A{}));
constexpr bool h() {
  typename A::X x = 1;
  return same<decltype(x), A::X<int>>;
}
static_assert(h());
