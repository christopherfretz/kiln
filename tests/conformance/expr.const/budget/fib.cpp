// [expr.const]/5.1 (implementation limits), [implimits]: the step count of
// recursive fib is clang 19's exactly: 6386 is the smallest
// -fconstexpr-steps clang accepts for fib(16) (fib-over-limit.cpp checks
// 6385 fails on both compilers). Memoized calls are charged their recorded
// steps and calls, so the counts do not depend on the memo.
//@ args: -fconstexpr-steps=6386
//@ oven: 12 steps<=354 calls<=177 bytes<=0
//@ oven: 13 steps<=6386 calls<=3193 bytes<=0
constexpr unsigned long long fib(unsigned n) {
  return n < 2 ? n : fib(n - 1) + fib(n - 2);
}
static_assert(fib(10) == 55);
static_assert(fib(16) == 987);
