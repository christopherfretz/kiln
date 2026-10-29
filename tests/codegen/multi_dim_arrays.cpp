// Two-dimensional arrays: locals, globals, indexing and decay.
int grid[3][4] = {{1, 2, 3, 4}, {5, 6, 7, 8}};
int row_sum(int (*row)[4]) { int s = 0; for (int i = 0; i < 4; i++) s += (*row)[i]; return s; }
int md(int i, int j) {
  int local[2][3] = {{1, 2, 3}, {4, 5, 6}};
  local[i & 1][j % 3] += grid[i % 3][j & 3];
  return local[1][2] + row_sum(&grid[1]) + grid[2][0];
}
