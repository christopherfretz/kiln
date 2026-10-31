// [temp.variadic]/5: expansion in a parenthesized initializer.
struct P { int a, b; constexpr P(int x, int y) : a(x), b(y) {} };
template <class... Ts> constexpr P make(Ts... ts) { P p(ts...); return p; }
static_assert(make(3, 4).b == 4);
