// Clang ext vectors, bool element type ([expr.const]; clang
// VectorExprEvaluator, `__builtin_shufflevector`): a shuffle of
// constexpr bool ext vectors folds lane by lane, also when it widens
// (`b4`, `b4` to 8 lanes) or narrows (`b8`, `b8` to 16 lanes, then read
// back), and a constexpr global initialized by one is a constant. clang
// 22 gives the shuffle the ext vector type of its lane count; clang 19's
// GNU-vector bit cast is bool-constexpr-shuffle-19.cpp. WP-438.
//@ expect: run
//@ requires: clang>=22
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef unsigned char u8;
typedef unsigned short u16;

constexpr b4 A = {true, false, false, true};
constexpr b4 B = {false, true, true, true};
constexpr b8 C = {false, true, true, false, false, false, false, true};

// Same lane count: the result has the operands' type.
static_assert(__builtin_bit_cast(u8, __builtin_shufflevector(C, C, 7, 6, 5, 4, 3, 2, 1, 0)) == 0x61);
static_assert(__builtin_bit_cast(u8, __builtin_shufflevector(C, (b8){}, 1, 1, 1, 1, 9, 9, 9, 9)) == 0x0f);
// Widening and wider-than-operands shuffles.
constexpr b8 W = __builtin_shufflevector(A, B, 0, 1, 2, 3, 4, 5, 6, 7);
static_assert(__builtin_bit_cast(u8, W) == 0xe9);
static_assert(__builtin_bit_cast(u8, (b8)__builtin_shufflevector(A, B, 7, 6, 5, 4, 3, 2, 1, 0)) == 0x97);
static_assert(__builtin_bit_cast(u16, (b16)__builtin_shufflevector(C, C, 1, 7, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)) == 0x8687);

int main() {
  constexpr u8 w = __builtin_bit_cast(u8, W);
  return w == 0xe9 ? 0 : 1;
}
