// [expr.prim.fold]/3: unary left and right folds.
template <class... Ts> constexpr int lsub(Ts... ts) { return (... - ts); }
template <class... Ts> constexpr int rsub(Ts... ts) { return (ts - ...); }
static_assert(lsub(10, 2, 3) == 5);
static_assert(rsub(10, 2, 3) == 11);
static_assert(lsub(7) == 7);
