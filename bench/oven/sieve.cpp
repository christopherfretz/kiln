//@ args: -stdlib=libc++ -fconstexpr-steps=200000000
// Sieve of Eratosthenes over a constexpr std::array<bool, N>: tight loops,
// array indexing, many small stores into one large object.
#include <array>
#include <cstddef>

template <std::size_t N>
constexpr std::size_t count_primes() {
  std::array<bool, N> composite{};
  std::size_t count = 0;
  for (std::size_t i = 2; i < N; ++i) {
    if (composite[i]) continue;
    ++count;
    for (std::size_t j = i * i; j < N; j += i) composite[j] = true;
  }
  return count;
}

static_assert(count_primes<100>() == 25);
static_assert(count_primes<50000>() == 5133);
