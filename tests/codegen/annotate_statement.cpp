// clang's `annotate` attribute on a statement (WP-1113,
// [dcl.attr.grammar]/5, [stmt.pre]/1): clang 22 accepts it and emits
// nothing for it (clang 19 rejects it).
//@ min-oracle: 22
int stmt(int x) {
  [[clang::annotate("st")]] x = 2;
  [[clang::annotate("st2")]] { x++; }
  [[clang::annotate("st3", 1)]] for (int i = 0; i < 2; ++i) x += i;
  [[clang::annotate("st4")]] return x;
}
