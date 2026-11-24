// The written-reference table (spec/ast.md §18.5), a pack-index-expression
// ([expr.prim.pack.index]): the pack's name in `ts...[0]` names the pack,
// as it does in the pack expansion `ts...`; only the indexing is
// dependent. Reduced from real code.
//@ written-refs: pack-index-expr.refs
//@ std: c++26
int g(int, int);
template <class... Ts> int first(Ts... ts) { return ts...[0]; }
template <class... Ts> int both(Ts... ts) { return g(ts...) + ts...[1]; }
template <class... Ts> int typed(Ts... ts) {
  Ts...[0] x = ts...[0];
  return x;
}
int use() { return first(1, 2) + both(3, 4) + typed(5); }
