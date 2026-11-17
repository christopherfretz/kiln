// [temp.param]/4, [temp.variadic]: the accepted side of a type-constraint's
// argument list: a pack expansion that lands on the concept's pack
// (`F<Us...>` for `template <class T, class... Us> concept F`, after
// fixed arguments, before more), an empty pack, a concept whose first
// parameter is the pack, a defaulted second parameter, and a variable's or
// return type's placeholder, whose list clang 22 checks only once it is
// deduced (GCC 16.1 rejects the return type's at the definition; kiln
// follows clang). WP-1610.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template <class T, class... Us> concept F = (sizeof...(Us) < 3);
template <class T, class U, class... Vs> concept H = sizeof(T) >= sizeof(U);
template <class... Ts> concept P = sizeof...(Ts) > 0;
template <class T, class U = int> concept D = sizeof(T) >= sizeof(U);
template <class T, class U> concept G = true;

template <class... Us> struct S {
  template <F<Us...> X> static constexpr int f1(X) { return 1; }
  template <F<int, Us...> X> static constexpr int f2(X) { return 2; }
  template <F<Us..., int> X> static constexpr int f3(X) { return 3; }
  template <H<int, Us...> X> static constexpr int f4(X) { return 4; }
  template <P<Us...> X> static constexpr int f5(X) { return 5; }
  static constexpr int f6(F<Us...> auto x) { return 6; }
  static constexpr G<Us...> auto f7() { return 7; }
  static constexpr int f8() { G<Us...> auto y = (Us(8) + ...); return y; }
};
template <D X> constexpr int d1(X) { return 9; }
constexpr int d2(D auto x) { return 10; }

static_assert(S<>::f1(0) == 1 && S<>::f2(0) == 2 && S<>::f3(0) == 3);
static_assert(S<char>::f1(0) == 1 && S<char, char>::f4(0L) == 4);
static_assert(S<>::f5(0) == 5 && S<int>::f6('a') == 6);
static_assert(S<int>::f7() == 7 && S<int>::f8() == 8);
static_assert(d1(0L) == 9 && d2(0) == 10);
