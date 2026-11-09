// [temp.func.order], [temp.deduct.partial]/11, [temp.deduct.type]/9,
// CWG1395/CWG1432: a function template whose parameter differs from
// another's only by a trailing pack (`f(L<T, U...>)` against `f(L<T>)`,
// `g(T, U...)` against `g(T, U)`) is less specialized, whether the pack
// is a function parameter pack, a template argument pack or one of a
// template template parameter's arguments (WP-1105).
//@ std: c++20
//@ expect: run
//@ stdout: f 221 g 2 h 2
extern "C" int printf(const char *, ...);
template <class... T> struct L {};
template <class T, class... U> constexpr int f(L<T, U...>) { return 1; }
template <class T> constexpr int f(L<T>) { return 2; }
template <class T, class U> constexpr int f(L<T, U>) { return 2; }
template <class T, class... U> constexpr int g(T, U...) { return 1; }
template <class T, class U> constexpr int g(T, U) { return 2; }
template <template <class...> class TT, class T, class... U> constexpr int h(TT<T, U...>) {
  return 1;
}
template <template <class...> class TT, class T> constexpr int h(TT<T>) { return 2; }

int main() {
  printf("f %d%d%d ", f(L<int>{}), f(L<int, int>{}), f(L<int, int, int>{}));
  printf("g %d ", g(1, 2));
  printf("h %d\n", h(L<int>{}));
}
