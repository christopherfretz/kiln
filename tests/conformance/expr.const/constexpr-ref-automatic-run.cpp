//@ expect: run
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
//@ stdout: 63
//@ stdout: 34
// [expr.const.init]/2 (C++26, P2686R5): constexpr references, pointers and
// structured bindings to automatic objects of the same function designate
// the objects of the current invocation at run time (a non-const referent
// stays writable through them); in a lambda they are odr-used and captured
// ([expr.const]/9: not constexpr-representable in the lambda body).
extern "C" int printf(const char *, ...);
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
struct H { const int *p; int n; };
int g(int parm) {
  constexpr P p{1, 2};
  constexpr auto &[x, y] = p;
  constexpr int arr[3] = {7, 8, 9};
  constexpr auto &[i, j, k] = arr;
  int m = 10;
  constexpr int &rm = m;
  rm += 5;
  constexpr int *pp = &parm;
  *pp *= 2;
  constexpr H h{&arr[1], 3};
  static_assert(*h.p == 8);
  return x + int(y) + i + j + k + m + parm + *h.p + h.n + (&x == &p.x) + (&k == arr + 2);
}
int l() {
  int m = 10;
  constexpr int &rm = m;
  constexpr int arr[2] = {1, 2};
  constexpr const int *pa = arr + 1;
  auto byref = [&] { return rm + *pa; };
  auto bycopy = [=] { return rm + *pa; };
  m = 20;
  return byref() + bycopy();
}
int main() {
  printf("%d\n", g(4));
  printf("%d\n", l());
}
