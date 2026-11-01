// [expr.type.conv]/2, [temp.variadic]/5: the parenthesized
// expression-list of a functional cast may contain pack expansions;
// after expansion one expression is a cast and several construct
// (libc++ <tuple> `tuple<_Tp&...>(__t...)`).
template <class... T> struct Tup {
  int n;
  constexpr Tup(T... t) : n((0 + ... + t)) {}
};
template <class... T> constexpr Tup<T...> make(T... t) { return Tup<T...>(t...); }
static_assert(make(1, 2, 3).n == 6);
static_assert(make(4).n == 4);

template <class U, class... T> constexpr U conv(T... t) { return U(t...); }
static_assert(conv<long>(7) == 7L);
static_assert(conv<double>(2) == 2.0);
struct P {
  int a, b;
  constexpr P(int x, int y) : a(x), b(y) {}
};
static_assert(conv<P>(1, 2).b == 2);

template <class... T> constexpr int sum(T... t) { return (0 + ... + int(t)); }
static_assert(sum(1.5, 2.5) == 3);
