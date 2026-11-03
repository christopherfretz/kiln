// Clang ext vectors, bool element type, clang 19 ([expr.const]; clang 19
// SemaBuiltinShuffleVector and BitCastBuffer): a `__builtin_shufflevector`
// whose lane count differs from its operands' has a GNU vector of `bool`
// type, implicitly bit-cast to the destination ext vector. clang 19's
// constant evaluator writes that GNU vector a byte per lane and reads the
// packed ext vector from the first of those bytes, so lane k of the result
// is lane k/8 of the shuffle when k % 8 == 0 and false otherwise (clang 22
// folds the shuffle itself: bool-constexpr-shuffle-run.cpp). A same-count
// shuffle keeps the operands' type and folds lane by lane. A narrowing
// shuffle to 4 lanes is rejected (bit_cast of 4 bool lanes). WP-438.
//@ expect: run
//@ requires: clang<22
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef unsigned char u8;
typedef unsigned short u16;

constexpr b4 A = {true, false, false, true};
constexpr b4 B = {false, true, true, true};
constexpr b8 C = {false, true, true, false, false, false, false, true};

static_assert(__builtin_bit_cast(u8, __builtin_shufflevector(C, C, 7, 6, 5, 4, 3, 2, 1, 0)) == 0x61);
static_assert(__builtin_bit_cast(u8, __builtin_shufflevector(C, (b8){}, 1, 1, 1, 1, 9, 9, 9, 9)) == 0x0f);
constexpr b8 W = __builtin_shufflevector(A, B, 0, 1, 2, 3, 4, 5, 6, 7);
static_assert(__builtin_bit_cast(u8, W) == 0x01);
static_assert(__builtin_bit_cast(u8, (b8)__builtin_shufflevector(A, B, 1, 6, 5, 4, 3, 2, 1, 0)) == 0x00);
static_assert(__builtin_bit_cast(u16, (b16)__builtin_shufflevector(C, C, 1, 7, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)) == 0x0101);
static_assert(__builtin_bit_cast(u16, (b16)__builtin_shufflevector(C, C, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)) == 0x0100);

int main() {
  constexpr u8 w = __builtin_bit_cast(u8, W);
  return w == 0x01 ? 0 : 1;
}
