//@ args: -fconstexpr-steps=100000000
// Oven allocation churn (spec/oven.md "Churn benchmarks"): 64 rounds of:
// grow an int buffer by doubling from 1 to 65536 elements (allocate the new
// buffer, copy, delete the old one), then free it. The heap peaks at old+new
// during the last copy (384 KiB). Only new/delete of scalars and aggregates.
// The gated copy is tests/conformance/expr.new/churn-vector.cpp.

// Grows a buffer from 1 to `limit` elements by doubling: allocate the new
// buffer, copy, delete the old one, fill the new half.
constexpr long long grow(int limit, int seed) {
  int cap = 1;
  int *buf = new int[1];
  buf[0] = seed;
  while (cap < limit) {
    int *next = new int[cap * 2];
    for (int i = 0; i < cap; ++i) next[i] = buf[i];
    delete[] buf;
    buf = next;
    for (int i = cap; i < cap * 2; ++i) buf[i] = buf[i - 1] + 1;
    cap *= 2;
  }
  long long last = buf[limit - 1];
  delete[] buf;
  return last;
}

constexpr long long churn(int rounds, int limit) {
  long long t = 0;
  for (int r = 0; r < rounds; ++r) t += grow(limit, r);
  return t;
}

// sum over r of (r + 65535)
static_assert(churn(64, 65536) == 64 * 65535 + 2016);
