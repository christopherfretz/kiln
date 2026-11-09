// [dcl.constexpr]/6: constexpr wrappers of the byte-swap, rotate and
// population-count builtins (the endian-helper pattern of widely used
// libraries) are possibly constant: clang 19 accepts them with
// -Winvalid-constexpr. kiln used to evaluate the call's callee operand
// (the builtin's function-to-pointer decay) on its own and report its
// "not supported yet" note as proof of never constant (WP-962).
//@ std: c++17
//@ args: -Winvalid-constexpr
//@ strict-diags
//@ expect: compile-ok
constexpr unsigned long long b(unsigned long long x) { return __builtin_bswap64(x); }
constexpr unsigned b32(unsigned x) { return __builtin_bswap32(x); }
constexpr unsigned short b16(unsigned short x) { return __builtin_bswap16(x); }
inline constexpr unsigned long long gbswap_64(unsigned long long host_int) {
  return __builtin_bswap64(host_int);
}
constexpr unsigned long long rl(unsigned long long x, int n) { return __builtin_rotateleft64(x, n); }
constexpr unsigned rr(unsigned x, int n) { return __builtin_rotateright32(x, n); }
constexpr int pc(unsigned long long x) { return __builtin_popcountll(x) + __builtin_popcount(unsigned(x)); }
constexpr int lz(unsigned x) { return x ? __builtin_clz(x) : 32; }
constexpr int tz(unsigned long long x) { return x ? __builtin_ctzll(x) : 64; }
static_assert(b(0x0102030405060708ull) == 0x0807060504030201ull, "");
static_assert(gbswap_64(1) == 0x0100000000000000ull, "");
static_assert(b32(0x01020304u) == 0x04030201u && b16(0x0102) == 0x0201, "");
static_assert(rl(1, 63) == 0x8000000000000000ull && rr(1, 1) == 0x80000000u, "");
static_assert(pc(0xffull) == 16 && lz(1) == 31 && tz(8) == 3, "");
int main() { return int(b(0)) + int(rl(0, 1)); }
