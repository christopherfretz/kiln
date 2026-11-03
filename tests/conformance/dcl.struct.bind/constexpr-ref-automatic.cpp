//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
// [dcl.struct.bind] with [expr.const.init]/2 (C++26, P2686R5): a constexpr
// structured binding declaration whose holding reference designates an
// automatic constexpr object of the same function is constant-initialized,
// and its bindings are usable in constant expressions there: aggregate,
// array and tuple-like (get<i> on the referent), in nested blocks and in a
// function template.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> requires requires { tuple_size<T>::value; }
struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
}
struct P { int x; long y; };
template <> struct std::tuple_size<P> { static constexpr size_t value = 2; };
template <> struct std::tuple_element<0, P> { using type = int; };
template <> struct std::tuple_element<1, P> { using type = long; };
template <std::size_t I> constexpr const auto &get(const P &p) {
  if constexpr (I == 0) return p.x; else return p.y;
}
struct S { int a, b; };
template <class T> constexpr int sum() {
  T t{3, 4};
  constexpr T u{5, 6};
  constexpr const auto &[a, b] = u;
  static_assert(a + b == 11);
  return a + b + t.a;
}
int g() {
  constexpr P p{1, 2};
  constexpr auto &[x, y] = p;
  static_assert(x == 1 && y == 2);
  static_assert(&x == &p.x);
  constexpr int arr[3] = {7, 8, 9};
  constexpr auto &[i, j, k] = arr;
  static_assert(i + j + k == 24 && &k == arr + 2);
  constexpr S s{1, 2};
  constexpr auto &[sa, sb] = s;
  static_assert(sa == 1 && &sb == &s.b);
  {
    { constexpr const auto &[m, n] = s; static_assert(n == 2); }
  }
  return sum<S>();
}
