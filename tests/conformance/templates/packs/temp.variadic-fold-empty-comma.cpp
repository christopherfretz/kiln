// [temp.variadic]/10: an empty unary fold of the comma operator is void().
template <class... Ts> constexpr void touch(Ts... ts) { (ts, ...); }
constexpr int f() { touch(); touch(1, 2); return 1; }
static_assert(f() == 1);
