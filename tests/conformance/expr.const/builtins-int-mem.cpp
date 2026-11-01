// [expr.const], [bit.rotate]: the GCC integer builtins `ffs`, `parity`,
// `clrsb`, `bitreverse` and `rotateleft`/`rotateright` (amount modulo the
// width), and the memory builtins `bcmp`, `wmemcpy` and `wmemmove`, are
// constant-evaluable (clang 19 `Constexpr` builtins).
//@ expect: compile-ok
//@ strict-diags
static_assert(__builtin_ffs(0) == 0);
static_assert(__builtin_ffs(8) == 4);
static_assert(__builtin_ffs(-1) == 1);
static_assert(__builtin_ffsl(0) == 0);
static_assert(__builtin_ffsll(1LL << 40) == 41);
static_assert(__builtin_parity(7u) == 1);
static_assert(__builtin_parityl(3ul) == 0);
static_assert(__builtin_parityll(~0ull) == 0);
static_assert(__builtin_clrsb(-1) == 31);
static_assert(__builtin_clrsb(0) == 31);
static_assert(__builtin_clrsbl(1) == 62);
static_assert(__builtin_clrsbll(-1LL << 62) == 1);
static_assert(__builtin_bitreverse8(1) == 0x80);
static_assert(__builtin_bitreverse16(0x00f1) == 0x8f00);
static_assert(__builtin_bitreverse32(1u) == 0x80000000u);
static_assert(__builtin_bitreverse64(0x10ul) == 0x0800000000000000ul);
static_assert(__builtin_rotateleft8(0x81, 1) == 0x03);
static_assert(__builtin_rotateleft16(1, 17) == 2);
static_assert(__builtin_rotateleft32(1u, -1) == 0x80000000u);
static_assert(__builtin_rotateleft64(1ul, 0) == 1);
static_assert(__builtin_rotateright8(1, 9) == 0x80);
static_assert(__builtin_rotateright16(1, 1) == 0x8000);
static_assert(__builtin_rotateright32(0x10u, 4) == 1);
static_assert(__builtin_rotateright64(1ul, 1) == 0x8000000000000000ul);

constexpr int cmp(const char *a, const char *b, decltype(sizeof 0) n) {
  return __builtin_bcmp(a, b, n);
}
static_assert(cmp("abc", "abd", 2) == 0);
static_assert(cmp("abc", "abd", 3) != 0);

constexpr int wide() {
  wchar_t a[4] = L"abc", b[4] = {};
  wchar_t *r = __builtin_wmemcpy(b, a, 3);
  __builtin_wmemmove(b + 1, b, 2);
  return (r == b) * 1000 + b[0] * 0 + (b[1] == L'a') * 10 + (b[2] == L'b');
}
static_assert(wide() == 1011);
