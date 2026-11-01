// [temp.deduct.call]/2 (WP-231): the top-level cv-qualifiers of a
// non-reference P are ignored for deduction, so `void f(const T t)`
// deduces `T = int` from an `int` argument, including when a trailing
// requires-clause names the parameter (`requires requires { t; }`).
//@ expect: run
//@ stdout: 1 2 3 4 5
#include <cstdio>
template <class T> int a(const T t) { return t; }
template <class T> int b(const T t) requires requires { t; } { return t + 1; }
template <class T> int c(const volatile T) { return 3; }
template <class T> int d(const T *const p) { return *p; }
template <class T> int e(T *const p) requires requires { requires sizeof(T) == sizeof(int); } { return *p + 1; }
template <class T> constexpr bool deduced_int(const T) { return __is_same(T, int); }
static_assert(deduced_int(1));
int main() {
  const int ci = 4;
  int x = 4;
  std::printf("%d %d %d %d %d\n", a(1), b(1), c(1L), d(&ci), e(&x));
}
