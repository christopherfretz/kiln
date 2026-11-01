// Constant evaluation of GNU and ext vector values ([expr.const]; clang 19
// VectorExprEvaluator): arithmetic, bitwise, shift, comparison and logical
// operators fold lane-wise, scalars splat, casts bit-reinterpret,
// __builtin_convertvector converts per lane, compound assignment and
// loops run in constexpr functions. Lanes are observed through
// __builtin_bit_cast to arrays (clang 19 rejects v[i] in constant
// expressions) (WP-173).
typedef int g4i __attribute__((vector_size(16)));
typedef unsigned g4u __attribute__((vector_size(16)));
typedef float g4f __attribute__((vector_size(16)));
typedef short g8s __attribute__((vector_size(16)));
typedef double g2d __attribute__((vector_size(16)));
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v3f __attribute__((ext_vector_type(3)));
struct I4 { int x[4]; };
struct F4 { float x[4]; };
struct S8 { short x[8]; };
constexpr I4 L(g4i v) { return __builtin_bit_cast(I4, v); }
constexpr bool eq(g4i v, int a, int b, int c, int d) {
  I4 s = L(v);
  return s.x[0] == a && s.x[1] == b && s.x[2] == c && s.x[3] == d;
}
constexpr bool feq(g4f v, float a, float b, float c, float d) {
  F4 s = __builtin_bit_cast(F4, v);
  return s.x[0] == a && s.x[1] == b && s.x[2] == c && s.x[3] == d;
}
constexpr g4i a = {1, 2, 3, 4};
constexpr g4i b = a + 1;
static_assert(eq(b, 2, 3, 4, 5));
static_assert(eq(a * b - a, 1, 4, 9, 16));
static_assert(eq(a / 2, 0, 1, 1, 2));
static_assert(eq(a % 3, 1, 2, 0, 1));
static_assert(eq(a << 2, 4, 8, 12, 16));
static_assert(eq(b >> a, 1, 0, 0, 0));
static_assert(eq((a & 1) | (a ^ 6), 7, 4, 5, 2));
static_assert(eq(-a, -1, -2, -3, -4));
static_assert(eq(~a, -2, -3, -4, -5));
static_assert(eq(+a, 1, 2, 3, 4));
static_assert(eq(!(a - 2), 0, -1, 0, 0));
static_assert(eq(a < 3, -1, -1, 0, 0));
static_assert(eq(a == b - 1, -1, -1, -1, -1));
// clang 19 folds a true lane of a vector && / || to 1 (-1 at run time).
static_assert(eq(a >= 2 && a <= 3, 0, 1, 1, 0));
static_assert(eq(a > 3 || a < 2, 1, 0, 0, 1));
static_assert(eq((g4i)(v4i)5, 5, 5, 5, 5));
static_assert(eq(true ? a : b, 1, 2, 3, 4));
static_assert(eq(false ? a : b, 2, 3, 4, 5));
static_assert(eq(((void)0, b), 2, 3, 4, 5));
constexpr g4i minv = {-2147483647 - 1, 0, 0, 0};
static_assert(eq(-minv, -2147483647 - 1, 0, 0, 0));
constexpr g4f f = __builtin_convertvector(a, g4f);
static_assert(feq(f, 1.0f, 2.0f, 3.0f, 4.0f));
static_assert(feq(f * 0.5f + f, 1.5f, 3.0f, 4.5f, 6.0f));
static_assert(feq(-f, -1.0f, -2.0f, -3.0f, -4.0f));
static_assert(eq(f < 2.5f, -1, -1, 0, 0));
static_assert(eq(__builtin_convertvector(f * 1.5f, g4i), 1, 3, 4, 6));
constexpr g4u u = (g4u)a;
static_assert(eq((g4i)(u - 2u), -1, 0, 1, 2));
constexpr g8s bc = (g8s)a;
static_assert(__builtin_bit_cast(S8, bc).x[0] == 1 && __builtin_bit_cast(S8, bc).x[1] == 0);
static_assert(__builtin_bit_cast(S8, bc).x[2] == 2);
static_assert(eq((g4i)(((__int128)7 << 64) | 9), 9, 0, 7, 0));
constexpr g4i step(g4i v) {
  g4i w = v;
  w += 1;
  w *= 2;
  w = w - v;
  w <<= 1;
  return w;
}
static_assert(eq(step(a), 6, 8, 10, 12));
constexpr g4i loop(int n) {
  g4i acc = {0, 0, 0, 0};
  for (int i = 0; i < n; ++i)
    acc = acc + (g4i){i, 1, 2 * i, -i};
  return acc;
}
static_assert(eq(loop(100), 4950, 100, 9900, -4950));
constexpr v4i e = {1, 2, 3, 4};
constexpr v4i e2 = e * e + 1;
static_assert(eq((g4i)e2, 2, 5, 10, 17));
constexpr v3f t = {1.0f, 2.0f, 3.0f};
constexpr v3f t2 = t + t;
constexpr g2d dd = {1.5, -2.0};
constexpr g2d dd2 = dd * dd;
struct D2 { double x[2]; };
static_assert(__builtin_bit_cast(D2, dd2).x[0] == 2.25 && __builtin_bit_cast(D2, dd2).x[1] == 4.0);
// A scalar condition with vector arms (scalars splat; CheckVectorOperands).
static_assert(eq(true ? a : 1, 1, 2, 3, 4));
static_assert(eq(false ? a : 1, 1, 1, 1, 1));
static_assert(eq(sizeof(a) == 16 ? 9 : a, 9, 9, 9, 9));
