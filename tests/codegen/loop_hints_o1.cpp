//@ args: -O1 -Xclang -disable-llvm-passes
//@ min-oracle: 22
// [cpp.pragma]/1, [stmt.iter]: loop-hint pragmas at -O1 (clang CGLoopInfo
// with optimization on): an `unroll_and_jam` outer loop moves its inner
// loop's metadata behind `llvm.loop.unroll_and_jam.followup_inner`, an
// explicit hint suppresses the implicit `unroll.disable`, a value of 0 or
// 1 after template substitution turns the hint into `disable`, and the
// followup chain (vectorize -> unroll) names the remaining transforms.
void f(int *a, int n) {
#pragma unroll_and_jam(4)
  for (int i = 0; i < n; ++i) {
#pragma clang loop vectorize(enable) unroll_count(2)
    for (int j = 0; j < n; ++j) a[j] += i;
    for (int j = 0; j < n; ++j) a[j] -= i;
  }
#pragma unroll_and_jam
  for (int i = 0; i < n; ++i)
#pragma clang loop unroll(full)
    for (int j = 0; j < 4; ++j) a[j] += i;
#pragma nounroll_and_jam
  for (int i = 0; i < n; ++i)
    for (int j = 0; j < n; ++j) a[j] += i;
#pragma unroll_and_jam(2)
  for (int i = 0; i < n; ++i)
#pragma clang loop unroll(enable) pipeline(disable) distribute(enable)
    while (n--) a[n] = i;
#pragma clang loop pipeline_initiation_interval(3) unroll(enable)
  for (int i = 0; i < n; ++i) a[i] = 0;
#pragma clang loop vectorize_predicate(enable) vectorize_width(1)
  for (int i = 0; i < n; ++i) a[i] = 0;
#pragma clang loop vectorize_width(2, fixed) interleave(disable) unroll_count(4)
  for (int i = 0; i < n; ++i) a[i] = 0;
#pragma clang loop distribute(enable) vectorize(disable)
  for (int i = 0; i < n; ++i) a[i] = 0;
#pragma unroll 1
  for (int i = 0; i < n; ++i) a[i] = 0;
#pragma unroll 0
  do { a[n] = 0; } while (--n);
#pragma unroll
  for (;;) {}
}
constexpr int K = 3;
int b[2];
template <int N, class T> void g(T *a) {
#pragma unroll N
  for (int i = 0; i < 16; ++i) a[i] = i;
#pragma clang loop unroll_count(N) vectorize_width(N)
  for (int i = 0; i < 16; ++i) a[i] = i;
#pragma clang loop interleave_count(N + K)
  for (T x : b) a[0] = x;
#pragma GCC unroll K
  for (int i = 0; i < 16; ++i) a[i] = i;
}
template void g<2, int>(int *);
template void g<1, int>(int *);
