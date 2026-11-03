// [cpp.pragma]/1 (implementation-defined pragmas), [stmt.iter]: clang's
// loop-hint pragmas (`#pragma unroll [N|(N)]`, `nounroll`,
// `unroll_and_jam`, `nounroll_and_jam`, `GCC unroll`, `#pragma clang loop
// <option>(<arg>)`) attach a LoopHintAttr to the following loop and become
// `!llvm.loop` metadata on its back-edge (clang CGLoopInfo). A hint value
// that depends on a template parameter is substituted at instantiation;
// `GCC ivdep` is not a loop hint (ignored pragma).
int a2[2];
void f(int *a, int n) {
#pragma unroll
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma unroll 4
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma unroll(8)
  while (n--) a[n] = 1;
#pragma nounroll
  do { a[n]++; } while (n++ < 10);
#pragma clang loop unroll(full)
  for (int i = 0; i < 8; ++i) a[i] = i;
#pragma clang loop unroll(disable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop unroll(enable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop unroll_count(3)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop vectorize(enable) interleave(enable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop vectorize(disable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop vectorize_width(4) interleave_count(2)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop interleave(disable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop distribute(enable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop distribute(disable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop pipeline(disable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop pipeline_initiation_interval(10)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma GCC unroll 4
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma GCC ivdep
  for (int i = 0; i < n; ++i) a[i] = i;
  for (int x : a2)
#pragma unroll 2
    for (int i = 0; i < n; ++i) a[i] = x;
#pragma clang loop vectorize_width(4, scalable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop vectorize_width(scalable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma clang loop vectorize_predicate(enable)
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma unroll_and_jam
  for (int i = 0; i < n; ++i) a[i] = i;
#pragma nounroll_and_jam
  for (int i = 0; i < n; ++i) a[i] = i;
}
template <int N> void g(int *a) {
#pragma unroll N
  for (int i = 0; i < 16; ++i) a[i] = i;
#pragma clang loop unroll_count(N) vectorize_width(N)
  for (int i = 0; i < 16; ++i) a[i] = i;
}
template void g<2>(int *);
