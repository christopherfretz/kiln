//@ args: -stdlib=libc++ -fconstexpr-steps=200000000
// Sorting a std::array in constant evaluation: a hand-written insertion
// sort (quadratic, element swaps through references) and std::sort from
// libc++ (iterators, comparators, deep inlined call chains), both checked
// against each other and against a closed-form checksum.
#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>

template <std::size_t N>
constexpr std::array<std::uint32_t, N> lcg_array(std::uint32_t seed) {
  std::array<std::uint32_t, N> a{};
  for (auto& x : a) {
    seed = seed * 1664525u + 1013904223u;
    x = seed >> 8;
  }
  return a;
}

template <std::size_t N>
constexpr std::array<std::uint32_t, N> insertion_sorted(std::array<std::uint32_t, N> a) {
  for (std::size_t i = 1; i < N; ++i)
    for (std::size_t j = i; j > 0 && a[j - 1] > a[j]; --j) std::swap(a[j - 1], a[j]);
  return a;
}

template <std::size_t N>
constexpr std::array<std::uint32_t, N> std_sorted(std::array<std::uint32_t, N> a) {
  std::sort(a.begin(), a.end());
  return a;
}

template <std::size_t N>
constexpr bool check(std::uint32_t seed) {
  auto in = lcg_array<N>(seed);
  auto a = insertion_sorted(in);
  auto b = std_sorted(in);
  std::uint64_t sum_in = 0, sum_out = 0;
  for (std::size_t i = 0; i < N; ++i) {
    sum_in += in[i];
    sum_out += a[i];
  }
  return a == b && std::is_sorted(a.begin(), a.end()) && sum_in == sum_out;
}

static_assert(check<16>(1));
static_assert(check<400>(42));

constexpr auto big = std_sorted(lcg_array<4000>(7));
static_assert(std::is_sorted(big.begin(), big.end()));
