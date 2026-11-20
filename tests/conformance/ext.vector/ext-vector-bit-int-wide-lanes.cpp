// [expr.const]: a vector of wide `_BitInt(N)` lanes (N > 128, a power of
// two), and a vector shifted by a wide count (whose count clang splats to
// such a vector), are evaluated lane by lane. x86-64 only (aarch64's
// `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ expect: compile-ok
//@ xfail(kiln): WP-1742 remainder, the Oven's vector lanes are registers, and a wide _BitInt is memory-class

using S = _BitInt(256);
typedef S v2 __attribute__((ext_vector_type(2)));
typedef int e4 __attribute__((ext_vector_type(4)));

constexpr v2 a = {1, 2};
constexpr v2 b = a + a;
static_assert(b[1] == 4 && b.x == 2);

constexpr e4 c = e4{1, 2, 3, 4} << S(1);
static_assert(c[3] == 8);
