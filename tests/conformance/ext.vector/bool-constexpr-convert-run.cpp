// Clang ext vectors, bool element type ([expr.const]; clang
// VectorExprEvaluator::VisitConvertVectorExpr): __builtin_convertvector
// folds from a bool ext vector (each lane becomes 0 or 1) and to one (a
// lane is true when the source lane is nonzero; a float lane converts
// toward zero and must be representable). WP-450.
//@ expect: run
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef signed char c8v __attribute__((ext_vector_type(8)));
typedef unsigned char uc8v __attribute__((ext_vector_type(8)));
typedef int i8v __attribute__((ext_vector_type(8)));
typedef float f8v __attribute__((ext_vector_type(8)));
typedef unsigned char u8;
typedef unsigned long long u64;

template <class V> constexpr u8 bits(V v) { return __builtin_bit_cast(u8, v); }

constexpr b8 A = {true, false, true, true, false, false, false, true};

constexpr c8v CA = __builtin_convertvector(A, c8v);
static_assert(__builtin_bit_cast(u64, CA) == 0x0100000001010001ull);
constexpr uc8v UA = __builtin_convertvector(!A, uc8v);
static_assert(__builtin_bit_cast(u64, UA) == 0x0001010100000100ull);

constexpr c8v C = {0, 3, 0, -1, 0, 0, 0, -128};
static_assert(bits(__builtin_convertvector(C, b8)) == 0x8a);
constexpr i8v I = __builtin_convertvector(A, i8v);
static_assert(bits(__builtin_convertvector(I, b8)) == 0x8d);
static_assert(bits(__builtin_convertvector(__builtin_convertvector(A, i8v) * 2, b8)) == 0x8d);

constexpr f8v F = {0.0f, 1.5f, 0.0f, -0.0f, 0.75f, 0.0f, 0.0f, 1.0f};
static_assert(bits(__builtin_convertvector(F, b8)) == 0x82);
constexpr f8v G = {0.0f, 1.0f, -0.5f, 1.25f, 0.25f, 1.5f, 0.0f, 0.0f};
static_assert(bits(__builtin_convertvector(G, b8)) == 0x2a);
constexpr f8v FA = __builtin_convertvector(A, f8v);
static_assert(bits(__builtin_convertvector(FA, b8)) == 0x8d);

constexpr b8 eq(i8v x, i8v y) { return __builtin_convertvector(x == y, b8); }
static_assert(bits(eq(I, (i8v){1, 1, 1, 1, 0, 0, 0, 0})) == 0x7d);

int main() {
  constexpr u8 r = bits(__builtin_convertvector(C, b8));
  constexpr u64 c = __builtin_bit_cast(u64, CA);
  return r == 0x8a && c == 0x0100000001010001ull ? 0 : 1;
}
