// __builtin_convertvector (clang SemaConvertVectorExpr) and the
// __builtin_reduce_* family (SemaBuiltinReduceMath / ReduceBitwise): the
// conversion keeps the element count and has the destination type; a
// reduction has the element type (WP-156).
typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned v4u __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef bool v4b __attribute__((ext_vector_type(4)));
typedef short g8s __attribute__((vector_size(16)));
typedef double g8d __attribute__((vector_size(64)));

v4i i;
v4f f;
v4b b;
g8s s;

static_assert(__is_same(decltype(__builtin_convertvector(i, v4f)), v4f));
static_assert(__is_same(decltype(__builtin_convertvector(f, v4b)), v4b));
static_assert(__is_same(decltype(__builtin_convertvector(b, v4u)), v4u));
static_assert(__is_same(decltype(__builtin_convertvector(s, g8d)), g8d));
static_assert(__is_same(decltype(__builtin_reduce_add(i)), int));
static_assert(__is_same(decltype(__builtin_reduce_mul(s)), short));
static_assert(__is_same(decltype(__builtin_reduce_and(b)), bool));
static_assert(__is_same(decltype(__builtin_reduce_or(i)), int));
static_assert(__is_same(decltype(__builtin_reduce_xor(s)), short));
static_assert(__is_same(decltype(__builtin_reduce_max(f)), float));
static_assert(__is_same(decltype(__builtin_reduce_min(i)), int));

// libc++ __algorithm/simd_utils.h: __all_of and the mask bit-cast.
template <class T, unsigned long N>
using vec __attribute__((__ext_vector_type__(N))) = T;
template <class T, unsigned long N>
bool all_of(vec<T, N> v) {
  return __builtin_reduce_and(__builtin_convertvector(v, vec<bool, N>));
}
template <class T, unsigned long N>
unsigned char mask(vec<T, N> v) {
  return __builtin_bit_cast(unsigned char, __builtin_convertvector(v, vec<bool, N>));
}
bool t1 = all_of(vec<int, 4>{1, 2, 3, 4});
unsigned char t2 = mask(vec<char, 8>{});
