// [expr.prim.pack.index]/1: a pack-index-expression is its id-expression
// through the `]` (spec/ast.md §21.2: it begins at its pattern), beside
// a pack expansion of the same pack (`ts...`) in a call.
//@ expect: compile-ok
//@ std: c++26
int g(int, int);
template <class... Ts> int z(Ts... ts) { return g(ts...) + ts...[0] + ts...[sizeof...(Ts) - 1]; }
template <class... Ts> int y() {
  Ts...[0] x = 0;
  return x;
}
int u = z(1, 2) + y<int, long>();
