// The comma operator ([expr.comma], [over.match.oper]): an overloaded
// `operator,` has an entry at each comma, also in a C-style cast's
// parenthesized operand, whose comma calls clang builds at the `(`
// (spec/ast.md §18.13.2). A built-in comma has none.
//@ written-refs: operators-comma.refs
struct M {};
int operator,(M, int);
M operator,(int, M);

void use(M m) {
  m, 1;
  (void)(m, 4);
  (int)(1, m, 2);
  int x = (m, 4);
  1, 2;
  (void)x;
}
