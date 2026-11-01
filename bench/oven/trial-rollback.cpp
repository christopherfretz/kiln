//@ args: -fconstexpr-steps=100000000
// Oven trial evaluation (spec/oven.md "Trial evaluation"): 20 000 failed
// __builtin_constant_p trials over a helper that writes a heap buffer made
// before the trial (logged, then restored), fills a local array, allocates
// and fails without freeing. Allocating fails a speculative trial (clang's
// rule) and every failure is rolled back, so the memory held stays O(1) in
// the number of trials: peak_arena is one buffer plus one helper frame,
// whatever the count. (Before trial evaluation, each failed trial leaked its
// 1 KiB block: peak_arena grew linearly and the evaluation failed.)

constexpr int helper(int *buf, int i) {
  int local[16] = {};
  for (int k = 0; k < 16; ++k) local[k] = buf[(i + k) % 64] + k;
  buf[i % 64] = local[i % 16];
  int *p = new int[256];
  p[0] = local[0];
  int z = 0;
  return p[0] / z;
}

constexpr int trials(int n) {
  int *buf = new int[64]{};
  int r = 0;
  for (int i = 0; i < n; ++i) r += __builtin_constant_p(helper(buf, i));
  delete[] buf;
  return r;
}

static_assert(trials(20000) == 0);
