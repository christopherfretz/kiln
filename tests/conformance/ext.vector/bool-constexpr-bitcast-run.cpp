// Clang ext vectors, bool element type ([bit.cast]; clang
// APValueToBufferConverter / BufferToAPValueConverter): a bool ext vector
// whose lane count is a multiple of 8 bit-casts to and from an object of
// its size, lane i being bit i%8 of byte i/8, and the round trip is the
// identity. WP-450.
//@ expect: run
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef bool b32 __attribute__((ext_vector_type(32)));
typedef bool b64 __attribute__((ext_vector_type(64)));
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned long long u64;

constexpr b8 A = {true, false, true, true, false, false, false, true};
static_assert(__builtin_bit_cast(u8, A) == 0x8d);
static_assert(__builtin_bit_cast(u8, __builtin_bit_cast(b8, (u8)0x5c)) == 0x5c);

constexpr b16 W = __builtin_bit_cast(b16, (u16)0x8001);
static_assert(__builtin_bit_cast(u16, W) == 0x8001);
constexpr b32 Y = __builtin_bit_cast(b32, 0x12345678u);
static_assert(__builtin_bit_cast(unsigned, Y) == 0x12345678u);
constexpr b64 X = __builtin_bit_cast(b64, 0x8000000000000005ull);
static_assert(__builtin_bit_cast(u64, X) == 0x8000000000000005ull);
static_assert(__builtin_bit_cast(u64, !X) == 0x7ffffffffffffffaull);

struct Pair { u8 lo, hi; };
constexpr Pair PW = __builtin_bit_cast(Pair, W);
static_assert(PW.lo == 0x01 && PW.hi == 0x80);
static_assert(__builtin_bit_cast(u16, __builtin_bit_cast(b16, Pair{0x34, 0x12})) == 0x1234);

struct Holder { b8 lanes; u8 tag; };
constexpr Holder H = {A, 7};
static_assert(__builtin_bit_cast(u16, H) == 0x078d);
static_assert(__builtin_bit_cast(u8, __builtin_bit_cast(Holder, (u16)0x0233).lanes) == 0x33);

constexpr u64 roundtrip(u64 v) { return __builtin_bit_cast(u64, __builtin_bit_cast(b64, v) ^ (b64)true); }
static_assert(roundtrip(0) == ~0ull);
static_assert(roundtrip(0x00ff00ff00ff00ffull) == 0xff00ff00ff00ff00ull);

int main() {
  constexpr u64 r = roundtrip(0x0123456789abcdefull);
  constexpr unsigned y = __builtin_bit_cast(unsigned, Y);
  return r == ~0x0123456789abcdefull && y == 0x12345678u ? 0 : 1;
}
