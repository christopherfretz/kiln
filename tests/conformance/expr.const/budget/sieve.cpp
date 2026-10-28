// [expr.const]/5.1, [implimits]: a sieve over a 2000-element local array;
// 9610 is the smallest -fconstexpr-steps clang 19 accepts
// (sieve-over-limit.cpp checks 9609 fails on both compilers).
//@ args: -fconstexpr-steps=9610
//@ oven: 17 steps<=452 calls<=1 bytes<=2000
//@ oven: 18 steps<=9610 calls<=1 bytes<=2000
constexpr int count_primes(int n) {
  bool composite[2000] = {};
  int count = 0;
  for (int i = 2; i < n; ++i) {
    if (composite[i]) continue;
    ++count;
    for (int j = i * i; j < n; j += i) composite[j] = true;
  }
  return count;
}
static_assert(count_primes(100) == 25);
static_assert(count_primes(2000) == 303);
