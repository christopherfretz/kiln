//@ expect: ast
//@ oracle: clang
// [expr.prim.fold]/1: unary left and right folds
template <class... Ts> auto sum_r(Ts... ts) { return (ts + ...); }
template <class... Ts> auto sum_l(Ts... ts) { return (... + ts); }
template <class... Ts> bool all(Ts... ts) { return (ts && ...); }
template <class... Ts> bool any(Ts... ts) { return (... || ts); }
template <class... Ts> void seq(Ts... ts) { (ts, ...); }
