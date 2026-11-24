// Operators and ud-suffixes in macro expansions (spec/ast.md §18.5,
// §18.13.4): an operator token from a macro body or argument keeps its
// token's location, so the dump shows its spelling and the outermost
// expansion (`exp=`). A ud-suffix in a macro argument spans its spelled
// bytes; one in a macro body or pasted by `##` too, but has no editable
// file range (§18.13.4).
//@ written-refs: operators-macros.refs
struct V {
  int operator[](int) const;
  V operator+(V) const;
};
int operator""_n(unsigned long long v) { return int(v); }
#define ADD(a, b) a + b
#define AT(v, i) v[i]
#define PLUS(a) a + a
#define LIT 3_n
#define ARG(x) x
#define PASTE(n) n##_n

int use(V v, V u) {
  ADD(v, u);
  AT(v, 2);
  PLUS(v);
  v ARG(+) u;
  return LIT + ARG(4_n) + PASTE(5);
}
