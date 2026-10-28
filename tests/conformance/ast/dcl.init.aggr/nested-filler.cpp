//@ expect: ast
struct In { int a; int b; };
struct Out { In in; int arr[4]; long tail; };
int use() {
  Out o = {{1}, {2, 3}};
  Out z = {};
  int grid[2][3] = {{1, 2, 3}, {4}};
  return o.in.a + z.arr[1] + grid[1][0];
}
