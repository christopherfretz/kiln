//@ args: -fconstexpr-steps=100000000
// fib.cpp without a header dependency (fib.cpp includes none either; this
// twin keeps the -nohdr set complete): naive doubly-recursive Fibonacci,
// pure call overhead. A memoizing evaluator collapses the recursion; see
// spec/oven.md "Memoization".

constexpr unsigned long long fib(unsigned n) {
  return n < 2 ? n : fib(n - 1) + fib(n - 2);
}

static_assert(fib(10) == 55);
static_assert(fib(26) == 121393);
