//@ args: -fconstexpr-steps=100000000
// Oven allocation churn (spec/oven.md "Churn benchmarks"): 16 rounds of:
// build a 64 KiB char string by 4096 appends of 16 bytes with 1.5x capacity
// growth, hash it, free it. Only new/delete of scalars and aggregates. The
// gated copy is tests/conformance/expr.new/churn-string.cpp.

struct Str {
  char *p;
  int len;
  int cap;
};

// Appends n bytes, growing the capacity by 1.5x (at least to fit).
constexpr void append(Str &s, const char *src, int n) {
  if (s.len + n > s.cap) {
    int cap = s.cap + s.cap / 2;
    if (cap < s.len + n) cap = s.len + n;
    char *p = new char[cap];
    for (int i = 0; i < s.len; ++i) p[i] = s.p[i];
    delete[] s.p;
    s.p = p;
    s.cap = cap;
  }
  for (int i = 0; i < n; ++i) s.p[s.len + i] = src[i];
  s.len += n;
}

// Builds a 64 KiB string from 4096 16-byte pieces and hashes it.
constexpr unsigned build(int pieces, unsigned seed) {
  Str s{nullptr, 0, 0};
  char piece[16] = {};
  for (int k = 0; k < pieces; ++k) {
    for (int i = 0; i < 16; ++i) piece[i] = static_cast<char>('a' + (k + i + seed) % 26);
    append(s, piece, 16);
  }
  unsigned h = 2166136261u;
  for (int i = 0; i < s.len; ++i) h = (h ^ static_cast<unsigned char>(s.p[i])) * 16777619u;
  delete[] s.p;
  return h;
}

constexpr unsigned churn(int rounds) {
  unsigned t = 0;
  for (int r = 0; r < rounds; ++r) t ^= build(4096, r) + r;
  return t;
}

static_assert(churn(16) != 0);
