// [stmt.ranged]/1: the AST of a range-based for over an array, with an
// init-statement and a nested loop (clang's CXXForRangeStmt children and
// its `__range1`/`__begin1`/`__end1` and depth-2 implicit variables).
//@ expect: ast
//@ ast-filter: sum_array

int sum_array() {
  int s = 0;
  for (int a[2][2] = {}; auto& row : a)
    for (const int& x : row) s += x;
  return s;
}
