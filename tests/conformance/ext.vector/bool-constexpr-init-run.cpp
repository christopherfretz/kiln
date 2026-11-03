// Clang ext vectors, bool element type ([expr.const]; clang
// VectorExprEvaluator, APValue::Vector): a constexpr bool ext vector is
// folded from a full or partial initializer list (missing lanes are
// false), an empty list, a scalar splat or a cast, and its lanes pack one
// bit per lane, lane i at bit i%8 of byte i/8. WP-450.
//@ expect: run
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef bool b32 __attribute__((ext_vector_type(32)));
typedef unsigned char u8;
typedef unsigned short u16;

template <class V> constexpr u8 bits(V v) { return __builtin_bit_cast(u8, v); }

constexpr b8 A = {true, false, true, true, false, false, false, true};
constexpr b8 P = {true, true};
constexpr b8 Z = {};
constexpr b8 V{};
constexpr b8 S = true;
constexpr b8 C = (b8)false;
constexpr b8 T = (b8)true;
constexpr b16 W = {true, false, false, false, false, false, false, false,
                   false, false, false, false, false, false, false, true};
constexpr b32 H = {false, false, false, false, false, false, false, false,
                   true};

static_assert(bits(A) == 0x8d);
static_assert(bits(P) == 0x03);
static_assert(bits(Z) == 0 && bits(V) == 0);
static_assert(bits(S) == 0xff && bits(T) == 0xff && bits(C) == 0);
static_assert(__builtin_bit_cast(u16, W) == 0x8001);
static_assert(__builtin_bit_cast(unsigned, H) == 0x100u);
static_assert(sizeof(b8) == 1 && sizeof(b16) == 2 && sizeof(b32) == 4);

constexpr b8 mk(bool x, bool y) { return (b8){x, y, x, y}; }
static_assert(bits(mk(true, false)) == 0x05);
static_assert(bits(mk(false, true)) == 0x0a);

int main() {
  constexpr u8 a = bits(A), m = bits(mk(true, true));
  return a == 0x8d && m == 0x0f ? 0 : 1;
}
