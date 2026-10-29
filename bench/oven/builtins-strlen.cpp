//@ args: -fconstexpr-steps=200000000
// __builtin_strlen, __builtin_memcmp and __builtin_char_memchr inside
// constant evaluation, as char_traits::length/compare/find (and so
// string_view) use them: repeated scans of long string literals and of a
// constexpr-built buffer (WP-088). Each call must be one Oven builtin, not an
// interpreted loop.

constexpr const char *lit =
    "the quick brown fox jumps over the lazy dog; THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG; "
    "pack my box with five dozen liquor jugs; PACK MY BOX WITH FIVE DOZEN LIQUOR JUGS; "
    "sphinx of black quartz, judge my vow; SPHINX OF BLACK QUARTZ, JUDGE MY VOW.";

constexpr long long scan(int rounds) {
  char buf[2048] = {};
  for (int i = 0; i < 2047; ++i)
    buf[i] = (char)('a' + i % 26);
  long long acc = 0;
  for (int r = 0; r < rounds; ++r) {
    acc += __builtin_strlen(lit) + __builtin_strlen(buf + r % 1024);
    acc += __builtin_memcmp(buf, buf + 26, 1000) == 0;
    const char *hit = __builtin_char_memchr(lit, 'V', 260);
    acc += hit ? hit - lit : -1;
  }
  return acc;
}

static_assert(scan(1) == 2367);
static_assert(scan(2000) == 3734424);
