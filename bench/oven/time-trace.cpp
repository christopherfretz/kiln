//@ mode: emit-llvm
//@ args: -ftime-trace=/dev/null -ftime-trace-verbose -fconstexpr-steps=200000000
// The traced cost of the Oven (spec/oven.md "Trace hook", WP-1115):
// 3000000 calls that cannot be memoized (each updates a caller's object
// through a reference), under `-ftime-trace -ftime-trace-verbose`, so
// every call takes the per-call hooks (two clock reads, below the
// default 500 us granularity, counted in `Total ConstexprCall`). The
// untraced cost is the rest of the oven group: their baselines are
// measured with no trace, so a hook that costs anything when off shows
// there. The trace goes to /dev/null so disk I/O stays out of the timing.

constexpr void bump(unsigned &x, unsigned i) { x = x * 31u + i; }

constexpr unsigned run(unsigned n) {
  unsigned x = 0;
  for (unsigned i = 0; i < n; ++i)
    bump(x, i);
  return x;
}

static_assert(run(3000000) == 637018976u);
