// [bit.count], [bit.byteswap] support: clz/ctz/popcount (all widths and the
// generic forms with and without a fallback), bswap, and calls folded to
// constants (clang EmitBuiltinExpr tries the evaluator first).
int a(unsigned x) { return __builtin_clz(x) + __builtin_ctz(x); }
int b(unsigned long x) { return __builtin_clzl(x) + __builtin_ctzll(x) + __builtin_popcountl(x); }
int c(unsigned short x, int f) { return __builtin_clzg(x) + __builtin_ctzg(x, f) + __builtin_clzg(x, 7); }
int d(unsigned char x) { return __builtin_popcountg(x) + __builtin_popcount(x); }
unsigned short e(unsigned short x) { return __builtin_bswap16(x); }
unsigned f(unsigned x) { return __builtin_bswap32(x); }
unsigned long g(unsigned long x) { return __builtin_bswap64(x); }
int h(unsigned __int128 x) { return __builtin_clzg(x, 0) + __builtin_popcountg(x); }
int k() { return __builtin_clz(1) + __builtin_popcount(255); }
long m(unsigned long long x) { return __builtin_ctzg(x); }
int n(unsigned long x) { return __builtin_clzg(x, -1) + __builtin_ctzg((unsigned char)x, 9); }
bool p(int x) { return __builtin_popcountg((unsigned)x) == 1; }
constexpr int q = __builtin_popcount(0xffu);
int r() { return q + __builtin_ctz(8) + __builtin_bswap32(1); }
