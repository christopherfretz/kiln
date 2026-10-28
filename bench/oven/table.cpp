//@ args: -stdlib=libc++ -fconstexpr-steps=200000000
// Table generation: build lookup tables (CRC-32, popcount, a small
// Pascal triangle) in constexpr functions returning std::array, then
// consume them from many constant expressions. Measures aggregate
// construction, copying of large results, and repeated table reads.
#include <array>
#include <cstddef>
#include <cstdint>
#include <utility>

constexpr std::array<std::uint32_t, 256> make_crc_table() {
  std::array<std::uint32_t, 256> t{};
  for (std::uint32_t i = 0; i < 256; ++i) {
    std::uint32_t c = i;
    for (int k = 0; k < 8; ++k) c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1;
    t[i] = c;
  }
  return t;
}

constexpr auto crc_table = make_crc_table();

constexpr std::uint32_t crc32(const char* s, std::size_t n, std::size_t rounds) {
  std::uint32_t c = 0xFFFFFFFFu;
  for (std::size_t r = 0; r < rounds; ++r)
    for (std::size_t i = 0; i < n; ++i)
      c = crc_table[(c ^ static_cast<unsigned char>(s[i])) & 0xFFu] ^ (c >> 8);
  return c ^ 0xFFFFFFFFu;
}

static_assert(crc32("123456789", 9, 1) == 0xCBF43926u);

constexpr std::array<std::uint8_t, 65536> make_popcount_table() {
  std::array<std::uint8_t, 65536> t{};
  for (std::size_t i = 1; i < t.size(); ++i) t[i] = static_cast<std::uint8_t>(t[i / 2] + (i & 1));
  return t;
}

constexpr auto popcount_table = make_popcount_table();
static_assert(popcount_table[0xFFFF] == 16 && popcount_table[0x5555] == 8);

constexpr std::uint64_t popcount_sum() {
  std::uint64_t s = 0;
  for (std::size_t i = 0; i < popcount_table.size(); ++i) s += popcount_table[i];
  return s;
}
static_assert(popcount_sum() == 16 * 32768);

template <std::size_t N>
constexpr std::array<std::array<std::uint64_t, N>, N> make_pascal() {
  std::array<std::array<std::uint64_t, N>, N> p{};
  for (std::size_t n = 0; n < N; ++n) {
    p[n][0] = 1;
    for (std::size_t k = 1; k <= n; ++k) p[n][k] = p[n - 1][k - 1] + p[n - 1][k];
  }
  return p;
}

constexpr auto pascal = make_pascal<64>();
static_assert(pascal[10][5] == 252);
static_assert(pascal[63][31] == 916312070471295267ull);

// Many independent evaluations reading the same table.
template <std::size_t I>
constexpr std::uint32_t crc_of_index() {
  char buf[4] = {char(I), char(I >> 8), char(I >> 16), char(I >> 24)};
  return crc32(buf, 4, 64);
}

template <std::size_t... Is>
constexpr std::uint64_t sum_all(std::index_sequence<Is...>) {
  return (std::uint64_t{crc_of_index<Is>()} + ...);
}

// 192 separate instantiations (clang's fold limit is 256 operands).
constexpr std::uint64_t all = sum_all(std::make_index_sequence<192>{});
static_assert(all == 412316860320ull);
