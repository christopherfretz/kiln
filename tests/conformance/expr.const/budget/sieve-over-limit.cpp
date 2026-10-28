// [expr.const]/5.1, [implimits]: one step below the minimum sieve.cpp needs:
// the evaluation stops at the step limit on both kiln and clang 19.
//@ args: -fconstexpr-steps=9609
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 20:15 note "in call to 'count_primes(2000)'"
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
