// Clang ext vectors ([expr.const]; clang VectorExprEvaluator and
// ScalarExprEmitter::VisitConvertVectorExpr): __builtin_convertvector of a
// `const` vector variable gives the same lanes whether the Oven evaluates
// it (a constexpr initializer) or codegen folds it (a runtime initializer,
// WP-490): to `bool` a lane is true when nonzero, from `bool` a lane is 0
// or 1, in both directions for integer, `float` and `bool` lanes. A float
// lane differs between the two as in clang 22: the evaluator converts it
// toward zero first (`0.5f` is false, `2.0f` is not a constant), the
// folded `fcmp une` compares it with zero (`0.5f`, `2.0f` and NaN are true).
//@ expect: run
//@ requires: clang>=22
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef int i8v __attribute__((ext_vector_type(8)));
typedef unsigned u8v __attribute__((ext_vector_type(8)));
typedef float f8v __attribute__((ext_vector_type(8)));
typedef unsigned char u8;

template <class V> constexpr u8 bits(V v) { return __builtin_bit_cast(u8, v); }

constexpr i8v I = {1, 0, -3, 0, 0, 7, 0, -1};
constexpr u8v U = {0u, 4000000000u, 1u, 0u, 0u, 0u, 2u, 0u};
constexpr f8v F = {1.5f, 0.0f, -0.0f, 1.0f, 0.0f, 1.0f, 0.0f, 1.25f};
constexpr b8 B = {true, false, false, true, true, false, true, false};

constexpr u8 CI = bits(__builtin_convertvector(I, b8));
constexpr u8 CU = bits(__builtin_convertvector(U, b8));
constexpr u8 CF = bits(__builtin_convertvector(F, b8));
constexpr i8v BI = __builtin_convertvector(B, i8v);
constexpr f8v BF = __builtin_convertvector(B, f8v);
static_assert(CI == 0xa5 && CU == 0x46 && CF == 0xa9);
constexpr f8v HALF = {0.5f, -0.25f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static_assert(bits(__builtin_convertvector(HALF, b8)) == 0);
static_assert(BI[0] == 1 && BI[1] == 0 && BI[3] == 1 && BF[6] == 1.0f && BF[7] == 0.0f);

int main() {
  b8 ri = __builtin_convertvector(I, b8);
  b8 ru = __builtin_convertvector(U, b8);
  b8 rf = __builtin_convertvector(F, b8);
  i8v bi = __builtin_convertvector(B, i8v);
  f8v bf = __builtin_convertvector(B, f8v);
  if (bits(ri) != CI || bits(ru) != CU || bits(rf) != CF)
    return 1;
  for (int k = 0; k < 8; ++k)
    if (bi[k] != BI[k] || bf[k] != BF[k])
      return 2;
  const f8v odd = {2.0f, __builtin_nanf(""), 0.5f, -0.25f, 0.0f, 0.0f, 0.0f, 0.0f};
  if (bits(__builtin_convertvector(odd, b8)) != 0x0f)
    return 3;
  b8 rh = __builtin_convertvector(HALF, b8);
  if (bits(rh) != 0x03)
    return 4;
  return 0;
}
