// Clang ext vectors, bool element type, [clang 20+] ([expr.const]; clang
// ExprConstant `__builtin_reduce_*`): the bitwise reductions of a bool
// ext vector fold to a bool, `and` being true only when every lane is
// true and `xor` giving the parity of the true lanes; `add` and `mul`
// wrap in the 1-bit lane type and `max`/`min` are `or`/`and`. WP-450.
//@ expect: run
//@ requires: clang>=22
typedef bool b3 __attribute__((ext_vector_type(3)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b24 __attribute__((ext_vector_type(24)));
typedef int i8v __attribute__((ext_vector_type(8)));

constexpr b8 A = {true, false, true, true, false, false, false, true};
constexpr b8 Z = {};
constexpr b8 T = (b8)true;

static_assert(__builtin_reduce_or(A) && !__builtin_reduce_and(A));
static_assert(!__builtin_reduce_xor(A));
static_assert(__builtin_reduce_xor(A ^ (b8){true}));
static_assert(!__builtin_reduce_or(Z) && !__builtin_reduce_and(Z));
static_assert(__builtin_reduce_and(T) && __builtin_reduce_or(T));
static_assert(!__builtin_reduce_add(A) && __builtin_reduce_add((b8){true}));
static_assert(!__builtin_reduce_mul(A) && __builtin_reduce_mul(T));
static_assert(__builtin_reduce_max(A) && !__builtin_reduce_min(A));

constexpr bool eq3(b3 a, b3 b) { return __builtin_reduce_and(a == b); }
static_assert(eq3((b3){true, false, true}, (b3){true, false, true}));
static_assert(!eq3((b3){true, false, true}, (b3){true, true, true}));

constexpr bool any_less(i8v x, int k) {
  return __builtin_reduce_or(__builtin_convertvector(x < k, b8));
}
static_assert(any_less((i8v){9, 9, 9, 3, 9, 9, 9, 9}, 4));
static_assert(!any_less((i8v){9, 9, 9, 9, 9, 9, 9, 9}, 4));

constexpr b24 every3() {
  b24 v = {};
  for (int i = 0; i < 24; i += 3) v[i] = true;
  return v;
}
static_assert(__builtin_reduce_or(every3()) && !__builtin_reduce_and(every3()));
static_assert(!__builtin_reduce_xor(every3()));

int main() {
  constexpr bool r = __builtin_reduce_or(A) && !__builtin_reduce_and(A) &&
                     any_less((i8v){1, 9, 9, 9, 9, 9, 9, 9}, 4);
  return r ? 0 : 1;
}
