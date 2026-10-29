//@ args: -fconstexpr-steps=200000000
// __builtin_memcpy / __builtin_memmove inside constant evaluation, the way
// libc++'s char_traits::copy/move and std::copy of trivial types use them:
// many block copies of a large int buffer, overlapping moves both ways, and
// a checksum (WP-088). The copies must run as Oven block moves, not as
// element-by-element interpreted loops.

constexpr long long churn(int rounds) {
  int a[4096] = {};
  int b[4096] = {};
  for (int i = 0; i < 4096; ++i)
    a[i] = i * 7 + 1;
  long long sum = 0;
  for (int r = 0; r < rounds; ++r) {
    __builtin_memcpy(b, a, sizeof a);
    __builtin_memmove(b + 1, b, (4096 - 1) * sizeof(int));
    __builtin_memmove(a, b + 2, (4096 - 2) * sizeof(int));
    sum += a[r % 4096] + b[(r * 31) % 4096];
  }
  return sum;
}

static_assert(churn(1) == 9);
static_assert(churn(400) == 7353082);
