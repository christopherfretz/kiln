// range-based for over arrays: by value, by reference, const reference and
// forwarding reference, a two-dimensional array, an init-statement,
// break/continue, and a range that is a reference to an array.
int sum(int (&a)[4]) {
  int s = 0;
  for (int x : a) s += x;
  return s;
}
void twice(long (&a)[3]) {
  for (auto& x : a) x *= 2;
}
double avg(const double (&a)[5]) {
  double s = 0;
  for (const auto& x : a) s += x;
  return s / 5;
}
int grid() {
  int m[2][3] = {{1, 2, 3}, {4, 5, 6}};
  int s = 0;
  for (auto&& row : m)
    for (int x : row) s = s * 2 + x;
  return s;
}
int jumps() {
  int s = 0;
  for (int a[] = {1, 2, 3, 4, 5}; int x : a) {
    if (x == 2) continue;
    if (x == 4) break;
    s += x;
  }
  return s;
}
