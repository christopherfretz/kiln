//@ args: -fconstexpr-steps=200000000
// table.cpp without libc++: the same CRC-32, popcount and Pascal-triangle
// tables, built into local arrays (no std::array, no class-type returns), and
// 192 CRC evaluations each rebuilding its table.

constexpr void make_crc_table(unsigned *t) {
  for (unsigned i = 0; i < 256; ++i) {
    unsigned c = i;
    for (int k = 0; k < 8; ++k) c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1;
    t[i] = c;
  }
}

constexpr unsigned crc32(const char *s, int n, int rounds) {
  unsigned t[256] = {};
  make_crc_table(t);
  unsigned c = 0xFFFFFFFFu;
  for (int r = 0; r < rounds; ++r)
    for (int i = 0; i < n; ++i) c = t[(c ^ (unsigned char)s[i]) & 0xFFu] ^ (c >> 8);
  return c ^ 0xFFFFFFFFu;
}

static_assert(crc32("123456789", 9, 1) == 0xCBF43926u);

constexpr unsigned long long popcount_sum() {
  unsigned char t[65536] = {};
  for (int i = 1; i < 65536; ++i) t[i] = (unsigned char)(t[i / 2] + (i & 1));
  if (t[0xFFFF] != 16 || t[0x5555] != 8) return 0;
  unsigned long long s = 0;
  for (int i = 0; i < 65536; ++i) s += t[i];
  return s;
}
static_assert(popcount_sum() == 16 * 32768);

constexpr unsigned long long pascal(int row, int col) {
  unsigned long long p[64][64] = {};
  for (int n = 0; n < 64; ++n) {
    p[n][0] = 1;
    for (int k = 1; k <= n; ++k) p[n][k] = p[n - 1][k - 1] + p[n - 1][k];
  }
  return p[row][col];
}
static_assert(pascal(10, 5) == 252);
static_assert(pascal(63, 31) == 916312070471295267ull);

constexpr unsigned crc_of_index(unsigned i) {
  char buf[4] = {(char)i, (char)(i >> 8), (char)(i >> 16), (char)(i >> 24)};
  return crc32(buf, 4, 64);
}

constexpr unsigned long long sum_all(unsigned n) {
  unsigned long long s = 0;
  for (unsigned i = 0; i < n; ++i) s += crc_of_index(i);
  return s;
}
static_assert(sum_all(192) == 412316860320ull);
