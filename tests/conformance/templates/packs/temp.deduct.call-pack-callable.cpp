// [temp.variadic]/5: forwarding a pack to a callable.
template <class F, class... A> constexpr auto call(F f, A... a) { return f(a...); }
constexpr int add(int a, int b) { return a + b; }
constexpr int zero() { return 0; }
static_assert(call(add, 3, 4) == 7);
static_assert(call(zero) == 0);
