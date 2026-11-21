// [expr.const]: a vector of wide `_BitInt(N)` lanes (N > 128, a power of
// two), and a vector shifted by a wide count (whose count clang splats to
// such a vector), are evaluated lane by lane: the operators, a lane
// subscript (read, write, `++`, compound assignment), an element access,
// `__builtin_convertvector` both ways, a shuffle, a splat, the reductions
// and the element-wise builtins. The comparisons are in
// ext-vector-bit-int-wide-lanes-compare.cpp, the failures in
// ext-vector-bit-int-wide-lanes-errors.cpp. x86-64 only (aarch64's
// `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ expect: compile-ok

using S = _BitInt(256);
using U = unsigned _BitInt(512);
typedef S v2 __attribute__((ext_vector_type(2)));
typedef U u4 __attribute__((ext_vector_type(4)));
typedef int e4 __attribute__((ext_vector_type(4)));
typedef int i2 __attribute__((ext_vector_type(2)));
typedef double d2 __attribute__((ext_vector_type(2)));

constexpr v2 a = {1, 2};
constexpr v2 b = a + a;
static_assert(b[1] == 4 && b.x == 2);
static_assert(a[1] == 2 && a.y == 2);

// A narrow vector shifted by a wide count.
constexpr e4 c = e4{1, 2, 3, 4} << S(1);
static_assert(c[3] == 8);

// Arithmetic past 128 bits, lane by lane.
constexpr S big = S(1) << 200;
static_assert((a * big)[1] == S(1) << 201);
static_assert((a << 200)[1] == S(1) << 201);
static_assert(((a << 254) >> 253)[0] == 2);
static_assert((v2{big, -big} / 2)[1] == -(S(1) << 199));
static_assert((v2{big + 5, 7} % v2{big, 4})[0] == 5);
static_assert((a ^ 3)[0] == 2 && (a | 4)[1] == 6 && (a & 3)[1] == 2);
static_assert((a - v2{2, big})[0] == -1);
static_assert((-a)[1] == -2 && (~a)[0] == -2 && (+a)[1] == 2);

// Unsigned lanes wrap.
static_assert((u4{0, 1, 2, 3} - 1)[0] == ~U(0));
static_assert((u4{~U(0), 1, 2, 3} + 1)[0] == 0);
static_assert((-u4{1, 0, 0, 0})[0] == ~U(0));

// Conversions.
static_assert(__builtin_convertvector(a, i2).y == 2);
static_assert(__builtin_convertvector(a, d2).y == 2.0);
static_assert(__builtin_convertvector(i2{-3, 4}, v2).x == -3);
static_assert(__builtin_convertvector(d2{-3.5, 4.5}, v2).y == 4);
typedef U u2 __attribute__((ext_vector_type(2)));
static_assert(__builtin_convertvector(v2{-1, 2}, u2)[0] == ~U(0));
static_assert(__builtin_convertvector(u2{~U(0), U(1) << 300}, v2)[0] == -1);
static_assert(__builtin_convertvector(u2{~U(0), U(1) << 300}, v2)[1] == 0);

// Shuffles and element accesses.
static_assert(__builtin_shufflevector(a, b, 3, 0)[0] == 4);
static_assert(__builtin_shufflevector(a, b, 3, 0)[1] == 1);
static_assert(__builtin_shufflevector(u4{1, 2, 3, 4}, u4{5, 6, 7, 8}, 7, 0).x == 8);
static_assert(v2{5, 6}.yx[0] == 6);
static_assert(u4{1, 2, 3, 4}.hi[1] == 4);
static_assert(u4{1, 2, 3, 4}.odd.x == 2);

// Reductions and element-wise builtins.
static_assert(__builtin_reduce_add(a) == 3);
static_assert(__builtin_reduce_max(v2{-big, 2}) == 2);
static_assert(__builtin_reduce_min(v2{-big, 2}) == -big);
static_assert(__builtin_reduce_mul(v2{3, big}) == 3 * big);
static_assert(__builtin_reduce_xor(a) == 3);
static_assert(__builtin_reduce_and(u4{~U(0), ~U(0) << 1, ~U(0), ~U(0)}) == ~U(0) << 1);
static_assert(__builtin_reduce_or(u4{1, 2, U(1) << 500, 0}) == (U(1) << 500) + 3);
static_assert(__builtin_elementwise_abs(v2{-big, 5})[0] == big);
static_assert(__builtin_elementwise_max(a, v2{0, 9})[1] == 9);
static_assert(__builtin_elementwise_min(a, v2{0, 9})[0] == 0);
static_assert(__builtin_elementwise_add_sat(u4{~U(0), 1, 2, 3}, u4{1, 1, 1, 1})[0] == ~U(0));
static_assert(__builtin_elementwise_sub_sat(u4{0, 1, 2, 3}, u4{1, 1, 1, 1})[0] == 0);

// Lane writes and whole-vector assignments.
constexpr bool write() { v2 v = {1, 2}; v[1] = 5; return v[1] == 5 && v[0] == 1; }
static_assert(write());
constexpr bool inc() { v2 v = {1, 2}; ++v[1]; v[0]--; return v[1] == 3 && v[0] == 0; }
static_assert(inc());
constexpr bool compound() {
  v2 v = {1, 2};
  v[1] += 3;
  v *= 2;
  v += 1;
  v <<= 2;
  v[0] <<= 100;
  return v[1] == 44 && v[0] == S(12) << 100;
}
static_assert(compound());
constexpr bool elem() { v2 v = {1, 2}; v.x = 9; v.y += v.x; return v.x == 9 && v.y == 11; }
static_assert(elem());
constexpr bool splat() { v2 v = big; v[1] += 1; return v[0] == big && v[1] == big + 1; }
static_assert(splat());
constexpr bool assign() {
  v2 v = {1, 2}, w;
  w = v;
  w.y = 7;
  return v.y == 2 && w.y == 7 && w.x == 1;
}
static_assert(assign());
constexpr S pick(bool c) { return (c ? v2{7, 8} : v2{9, 10})[1]; }
static_assert(pick(true) == 8 && pick(false) == 10);
