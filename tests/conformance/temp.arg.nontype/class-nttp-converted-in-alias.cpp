// [temp.arg.nontype]/1, [temp.alias]/2: a value-dependent argument `X`
// (an int) for a class-type template parameter `n N` of an alias
// template is converted to `n`, so `N.i` in the alias is a member access
// on an `n`. Found running GCC's C++ testsuite (WP-1472).
//@ expect: compile-ok
//@ std: c++20
struct n {
  constexpr n(int a) : i(a) {}
  int i;
};
template <n N> using i_type = decltype(N.i);
template <int X> int f() {
  using T = i_type<X>;
  static_assert(sizeof(T) == sizeof(int));
  return T{};
}
template int f<3>();
