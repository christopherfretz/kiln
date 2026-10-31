// [stmt.ranged]: nested range-based for statements over a two-dimensional
// array (the outer loop variable is a reference to a row), and a range
// that is a reference to an array.
//@ expect: run
//@ stdout: 21 7
extern "C" int printf(const char*, ...);

int total(const int (&m)[2][3]) {
  int s = 0;
  for (const auto& row : m)
    for (int x : row) s += x;
  return s;
}

int main() {
  int m[2][3] = {{1, 2, 3}, {4, 5, 6}};
  int (&r)[2][3] = m;
  int rows = 0;
  for (auto& row : r) rows += row[0] + row[1] - row[2] + 1;
  printf("%d %d\n", total(m), rows + 2);
}
