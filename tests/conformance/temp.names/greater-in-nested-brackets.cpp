// [temp.names]/4: a '>' inside parentheses, brackets or braces within a template argument is
// an operator, not the end of the argument list (decltype, noexcept, casts, array bounds,
// braced lists, explicit-specifiers, noexcept-specifiers, requires-expressions)
//@ expect: compile-ok
//@ strict-diags
template <bool B> struct B_ { static constexpr bool value = B; };
template <class T> T&& declval();
struct X { int a; };
template <int N> struct I { static constexpr int v = N; };
static_assert(B_<noexcept(1 > 2)>::value);
static_assert(I<sizeof(char[3 > 2 ? 4 : 1])>::v == 4);
static_assert(I<alignof(char) + (2 > 1)>::v == 2);
static_assert(B_<__is_same(decltype(1 > 2), bool)>::value);
static_assert(I<int(3 > 2)>::v == 1);
static_assert(I<static_cast<int>(3 > 2)>::v == 1);
static_assert(I<X{3 > 2}.a>::v == 1);
template <class T> struct Q { explicit(sizeof(T) > 1) Q(int) {} };
template <class T> void nx() noexcept(sizeof(T) > 1) {}
static_assert(B_<noexcept(nx<int>())>::value && !B_<noexcept(nx<char>())>::value);
static_assert(B_<requires { 1 > 2; }>::value);
template <class T, class U> constexpr bool conv = true;
template <bool B, class T = void> struct E { using type = T; };
template <class T> struct E<false, T> {};
template <class T, class U>
typename E<conv<decltype(declval<const T&>() > declval<const U&>()), bool>, bool>::type gt(T, U) {
  return true;
}
bool r = gt(1, 2);
