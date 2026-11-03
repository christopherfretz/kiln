//@ expect: ast
//@ ast-filter: f
//@ oracle: clang
// [cpp.pragma]/1 (clang extension): `#pragma unroll`, `#pragma nounroll`,
// `#pragma GCC unroll` and `#pragma clang loop` hints become LoopHint
// attributes of an AttributedStmt around the loop they precede, one per
// hint in pragma order and before the statement's own attributes (clang
// `ParsePragmaLoopHint`); a numeric hint keeps its value expression.
int b[2];
int f(int *a, int n) {
#pragma unroll
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma unroll 4
#pragma clang loop vectorize(enable) interleave_count(2)
  while (n--) a[n] = 1;
#pragma nounroll
  do { a[n]++; } while (n++ < 10);
#pragma GCC unroll(8)
  [[likely]] for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop vectorize_width(4, scalable)
  for (int x : b) a[0] += x;
  return n;
}
