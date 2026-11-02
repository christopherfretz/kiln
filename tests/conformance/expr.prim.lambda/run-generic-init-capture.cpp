// [expr.prim.lambda.capture]/6, [temp.inst]/5: the body of a generic
// lambda names its init-capture variables, and each specialization of the
// call operator (or of a generic lambda nested in it) uses the closure's
// copy, in a plain function and in a function template (libc++
// `basic_format_arg::visit` test visitors `[v = To(value)](auto a)`;
// WP-286). Matches clang 19.
//@ expect: run
//@ stdout: 3 114 16 6
#include <cstdio>
#include <type_traits>
template <class F> auto inv(F f) -> decltype(f(true)) { return f(true); }
template <class To, class From>
int visit(From value) {
  auto visitor = [v = To(value)](auto a) {
    if constexpr (std::is_same_v<To, decltype(a)>)
      return v == a ? 3 : 1;
    else
      return 2;
  };
  return inv(visitor);
}
template <class T>
int nested(T t) {
  auto g = [v = t * 2, &r = t](auto a) {
    auto inner = [v, &r](auto b) { return v + r + b; };
    r += 1;
    return inner(a);
  };
  return g(100) + t;
}
int plain(int x) {
  auto g = [w = x + 1](auto a) mutable { w += a; return w; };
  g(1);
  return g(10);
}
int twice(int x) {
  auto g = [w = x](auto a, auto b) { return w + a + b; };
  return g(1, 2L) + g(short(1), 'a' - 'a') - x - 1;
}
int main() {
  std::printf("%d %d %d %d\n", visit<bool>(true), nested(3), plain(4), twice(3));
}
