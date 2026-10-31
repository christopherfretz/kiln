// [expr.prim.fold]: a comma fold evaluates its operands left to right.
template <class... Ts> constexpr int digits(Ts... ts) {
  int r = 0;
  ((r = r * 10 + ts), ...);
  return r;
}
static_assert(digits(1, 2, 3) == 123);
