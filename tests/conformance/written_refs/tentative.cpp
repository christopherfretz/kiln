// The written-reference table (spec/ast.md §18.5): names that the
// parser's disambiguation looks at before it parses them ([stmt.ambig],
// [dcl.ambig.res], a `<` after a name, a parenthesized type or
// expression) are recorded once, by the parse that is kept; the
// abandoned reading leaves nothing.
//@ written-refs: tentative.refs
//@ args: -Wno-vexing-parse
struct T {
  T();
  T(int);
  int operator()(int) const;
};
int y = 1;
int (*fn)(int);
template <int N> int tv(int);
int a = 4, b = 5;

void tentative() {
  T(q);                // a declaration of `q` ([stmt.ambig]/1): `T` only
  T(r)(y);             // a declaration of `r` initialized by `y`
  T(y + 1)(y);         // an expression: `T` and two `y`
  int z(y);            // a variable initialized by `y`
  int w(int(y));       // a function declaration ([dcl.ambig.res]/1)
  int s = sizeof(y) + (fn)(y) + tv<3>(y) + (a < b) + (b > y);
  (void)q;
  (void)r;
  (void)z;
  (void)s;
}
