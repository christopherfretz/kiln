// [temp.names]/5, [expr.ref]/1, CWG1835: in `e.template B<T>::m` the
// `template` keyword introduces the qualifier's template-id `B<T>`, not
// the member name `m`; kiln took `m` for the template name and reported
// "'m' following the 'template' keyword does not refer to a template"
// (stdexec's `__tuple` apply, `self.template __box<Ts, I>::__value...`;
// WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 6 6 9 7
#include <cstdio>

template <class T, unsigned I> struct box { T value; };
template <unsigned... I> struct idx {};
template <class Ix, class... Ts> struct base;
template <unsigned... I, class... Ts> struct base<idx<I...>, Ts...> : box<Ts, I>... {
  template <class F, class Self> static constexpr auto apply(F&& f, Self&& self) -> decltype(auto) {
    return static_cast<F&&>(f)(static_cast<Self&&>(self).template box<Ts, I>::value...);
  }
};
struct same_types : base<idx<0, 1, 2>, int, long, int> {};
struct distinct : base<idx<0, 1, 2>, int, long, char> {};

template <class T> int second(T& t) { return t.template box<long, 1>::value; }

int main() {
  same_types s{{{1}, {2}, {3}}};
  distinct d{{{1}, {2}, {3}}};
  auto sum = [](auto a, auto b, auto c) { return int(a + b + c); };
  std::printf("%d %d %d %d\n", same_types::apply(sum, s), distinct::apply(sum, d),
              base<idx<0, 1>, int, int>::apply([](int a, int b) { return a + b; },
                                              base<idx<0, 1>, int, int>{{4}, {5}}),
              second(d) + 5);
}
