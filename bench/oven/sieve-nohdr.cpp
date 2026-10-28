//@ args: -fconstexpr-steps=200000000
// sieve.cpp without libc++: the same Sieve of Eratosthenes over a plain
// local bool array instead of std::array (tight loops, indexing, many small
// stores into one large object), so kiln can run it before it compiles
// libc++.

constexpr int count_primes(int n) {
  bool composite[50000] = {};
  int count = 0;
  for (int i = 2; i < n; ++i) {
    if (composite[i]) continue;
    ++count;
    for (long long j = (long long)i * i; j < n; j += i) composite[j] = true;
  }
  return count;
}

static_assert(count_primes(100) == 25);
static_assert(count_primes(50000) == 5133);
