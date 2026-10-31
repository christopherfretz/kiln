// [expr.prim.fold]: the pattern may be any cast-expression naming the pack.
template <class... Ts> constexpr int sq(Ts... ts) { return ((ts * ts) + ... + 0); }
static_assert(sq(1, 2, 3) == 14);
template <class... Ts> constexpr unsigned sizes() { return (sizeof(Ts) + ...); }
static_assert(sizes<char, short, int>() == 1 + sizeof(short) + sizeof(int));
