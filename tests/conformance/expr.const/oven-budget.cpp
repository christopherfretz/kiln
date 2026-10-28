// [expr.const] + kiln Oven budgets (spec/oven.md): an iterative fib(40)
// must stay within a small evaluation budget. The numbers are provisional
// until the Oven reports its counters; the harness parses but cannot yet
// enforce them, and other compilers skip the test.
//@ oven: steps<=5000 bytes<=1024
constexpr unsigned long long fib(int n) {
  unsigned long long a = 0, b = 1;
  for (int i = 0; i < n; ++i) {
    unsigned long long t = a + b;
    a = b;
    b = t;
  }
  return a;
}

static_assert(fib(40) == 102334155);
