//@ args: -fconstexpr-steps=100000000
// Naive doubly-recursive Fibonacci: pure call overhead, small frames, no
// memory traffic. Measures the evaluator's per-call cost. (A memoizing
// evaluator may legitimately collapse this; see spec/oven.md "Memoization".)

constexpr unsigned long long fib(unsigned n) {
  return n < 2 ? n : fib(n - 1) + fib(n - 2);
}

static_assert(fib(10) == 55);
static_assert(fib(26) == 121393);
