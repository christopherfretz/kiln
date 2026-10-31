// [expr.prim.fold]/3: a binary right fold over a deduced function parameter pack.
template <class... Ts> constexpr int sum(Ts... ts) { return (ts + ... + 0); }
static_assert(sum(1, 2, 3) == 6);
static_assert(sum() == 0);
static_assert(sum(5) == 5);
