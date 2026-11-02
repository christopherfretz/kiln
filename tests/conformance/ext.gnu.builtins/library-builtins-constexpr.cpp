// The *f128 math builtins (GNU; on this target their type is the IEEE
// quad `long double`) evaluate like their `l` twins in constant
// expressions where clang 19 folds them (fabs, copysign, fmax, fmin, inf,
// huge_val, nan); __builtin_assume_aligned is its pointer argument
// ([expr.const]). __builtin_shufflevector lanes come from the
// concatenated operands in index order. Verified against clang 19.
//@ expect: compile-ok
using Q = long double;
constexpr Q a = __builtin_fabsf128(-2.5L);
static_assert(a == 2.5L);
static_assert(__builtin_copysignf128(3.0L, -1.0L) == -3.0L);
static_assert(__builtin_huge_valf128() == __builtin_inff128());
static_assert(__builtin_nanf128("") != __builtin_nanf128(""));
static_assert(__builtin_fmaxf128(1.0L, 2.0L) == 2.0L);
static_assert(__builtin_fminf128(1.0L, 2.0L) == 1.0L);
static_assert(__builtin_isinf(__builtin_inff128()));
static_assert(sizeof(__builtin_fabsf128(1)) == 16);
constexpr void *p = __builtin_assume_aligned((void *)nullptr, 16);
static_assert(p == nullptr);
constexpr int arr[4] = {};
static_assert(__builtin_assume_aligned(arr, 4, 0) == arr);

typedef int v4 __attribute__((ext_vector_type(4)));
struct S4 { int a, b, c, d; };
struct S2 { int a, b; };
constexpr v4 x = {1, 2, 3, 4};
constexpr v4 y = {5, 6, 7, 8};
constexpr S4 c = __builtin_bit_cast(S4, __builtin_shufflevector(x, y, 0, 5, 2, 7));
static_assert(c.a == 1 && c.b == 6 && c.c == 3 && c.d == 8);
constexpr S2 d = __builtin_bit_cast(S2, __builtin_shufflevector(x, y, 7, 0));
static_assert(d.a == 8 && d.b == 1);
template <int I> constexpr int lane(v4 v) {
  return __builtin_bit_cast(S2, __builtin_shufflevector(v, v, I, I)).a;
}
static_assert(lane<3>(x) == 4 && lane<0>(y) == 5);
