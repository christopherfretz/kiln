// [expr.prim.fold]: fold operators include compound assignment.
template <class... Ts> constexpr int acc(Ts... ts) {
  int r = 0;
  (r += ... += ts);
  return r;
}
static_assert(acc(1, 2, 3, 4) == 10);
