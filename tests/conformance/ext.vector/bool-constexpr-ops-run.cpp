// Clang ext vectors, bool element type ([expr.const]; clang
// VectorExprEvaluator): `!`, `~`, unary `+`/`-`, `&`, `|`, `^`
// (vector and scalar operands) and `==`/`!=` fold lane by lane on a
// constexpr bool ext vector, giving a bool ext vector. WP-450.
//@ expect: run
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef unsigned char u8;
typedef unsigned short u16;

template <class V> constexpr u8 bits(V v) { return __builtin_bit_cast(u8, v); }

constexpr b8 A = {true, false, true, true, false, false, false, true};
constexpr b8 B = {true, true, false, true, false, true, false, false};

static_assert(bits(!A) == 0x72);
static_assert(bits(~A) == 0x72);
static_assert(bits(+A) == 0x8d);
static_assert(bits(-A) == 0x8d);
static_assert(bits(A & B) == 0x09);
static_assert(bits(A | B) == 0xaf);
static_assert(bits(A ^ B) == 0xa6);
static_assert(bits(A & true) == 0x8d);
static_assert(bits(A ^ true) == 0x72);
static_assert(bits(false | A) == 0x8d);
static_assert(bits(A == B) == 0x59);
static_assert(bits(A != B) == 0xa6);
static_assert(bits(A == A) == 0xff);

constexpr b16 W = __builtin_bit_cast(b16, (u16)0xa55a);
static_assert(__builtin_bit_cast(u16, ~W) == 0x5aa5);
static_assert(__builtin_bit_cast(u16, W ^ (b16)true) == 0x5aa5);
static_assert(__builtin_bit_cast(u16, W & !W) == 0);

constexpr b8 sel(b8 m, b8 x, b8 y) { return (m & x) | (!m & y); }
static_assert(bits(sel(A, B, !B)) == 0x59);

int main() {
  constexpr u8 x = bits(A ^ B), s = bits(sel(A, B, !B));
  return x == 0xa6 && s == 0x59 ? 0 : 1;
}
