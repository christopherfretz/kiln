// [expr.prim.fold]/3: a binary left fold associates to the left.
template <class... Ts> constexpr int sub(Ts... ts) { return (100 - ... - ts); }
static_assert(sub(1, 2, 3) == 94);
template <class... Ts> constexpr int rsub(Ts... ts) { return (ts - ... - 100); }
static_assert(rsub(1, 2, 3) == 1 - (2 - (3 - 100)));
