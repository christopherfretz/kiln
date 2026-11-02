// [expr.prim.lambda.closure]/3, [temp.variadic]/5, [expr.const]: folds in a
// lambda with an explicit template-parameter-list are constant-evaluated,
// including one that mixes the enclosing function's parameter pack with the
// lambda's own pack, and a captured pack.
//@ expect: compile-ok
template <unsigned long... I> struct seq {};
template <class... Idx> constexpr long dot(const long (&s)[3], Idx... idx) {
  return [&]<unsigned long... P>(seq<P...>) {
    return ((static_cast<long>(idx) * s[P]) + ... + 0L);
  }(seq<0, 1, 2>());
}
constexpr long strides[3] = {10, 20, 30};
static_assert(dot(strides, 1, 2, 3) == 140);
static_assert([]<class... Ts>(Ts... ts) { return (ts * ...); }(2, 3, 4) == 24);
template <class... Ts> constexpr long cap(Ts... ts) {
  return [ts...]<class... Us>(Us... us) { return ((ts - us) + ...); }(1, 1);
}
static_assert(cap(5, 7) == 10);
