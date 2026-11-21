// [expr.const]: the failures of an operation on a vector of wide
// `_BitInt(N)` lanes (N > 128), noted as clang 22 notes them: a read of a
// non-constexpr vector, a lane's overflow, division by zero and shift
// count (with the vector type and its lanes), a reduction's overflow, a
// lane read one past the end, a lane `++` and compound assignment
// overflowing, and a read of the indeterminate lane past the end of an
// odd-sized vector's `.hi`. x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is
// 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:15 note "read of non-constexpr variable 'g' is not allowed in a constant expression"
//@ diag: 46:4 note "declared here"
//@ diag: 50:13 error "constexpr variable 'add' must be initialized by a constant expression"
//@ diag: 50:22 note "value 57896044618658097711785492504343953926634992332820282019728792003956564819968 is outside the range of representable values of type 'v2' (vector of 2 'S' values)"
//@ diag: 51:13 error "constexpr variable 'quo' must be initialized by a constant expression"
//@ diag: 51:36 note "value 57896044618658097711785492504343953926634992332820282019728792003956564819968 is outside the range of representable values of type 'v2' (vector of 2 'S' values)"
//@ diag: 52:13 error "constexpr variable 'zero' must be initialized by a constant expression"
//@ diag: 52:23 note "division by zero"
//@ diag: 53:13 error "constexpr variable 'shl' must be initialized by a constant expression"
//@ diag: 53:22 note "negative shift count -1"
//@ diag: 54:13 error "constexpr variable 'shr' must be initialized by a constant expression"
//@ diag: 54:22 note "shift count 256 >= width of type 'v2' (vector of 2 'S' values) (256 bits)"
//@ diag: 55:13 error "constexpr variable 'red' must be initialized by a constant expression"
//@ diag: 55:19 note "value 57896044618658097711785492504343953926634992332820282019728792003956564819968 is outside the range of representable values of type 'S' (aka '_BitInt(256)')"
//@ diag: 56:13 error "constexpr variable 'mul' must be initialized by a constant expression"
//@ diag: 56:19 note "value 115792089237316195423570985008687907853269984665640564039457584007913129639934 is outside the range of representable values of type 'S' (aka '_BitInt(256)')"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:44 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 59:15 note "in call to 'past()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:38 note "value 57896044618658097711785492504343953926634992332820282019728792003956564819968 is outside the range of representable values of type 'S' (aka '_BitInt(256)')"
//@ diag: 61:15 note "in call to 'inc()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:46 note "value 57896044618658097711785492504343953926634992332820282019728792003956564819968 is outside the range of representable values of type 'S' (aka '_BitInt(256)')"
//@ diag: 63:15 note "in call to 'compound()'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"

using S = _BitInt(256);
typedef S v2 __attribute__((ext_vector_type(2)));
typedef S v3 __attribute__((ext_vector_type(3)));
constexpr S max = S((unsigned _BitInt(256))-1 >> 1);

v2 g = {1, 2};
static_assert(g[1] == 2);

constexpr v2 a = {1, max};
constexpr S add = (a + 1)[1];
constexpr S quo = (v2{-max - 1, 0} / -1)[0];
constexpr S zero = (a % v2{1, 0})[1];
constexpr S shl = (a << -1)[0];
constexpr S shr = (a >> 256)[0];
constexpr S red = __builtin_reduce_add(a);
constexpr S mul = __builtin_reduce_mul(v2{2, max});

constexpr S past() { v2 v = {1, 2}; return v[2]; }
static_assert(past() == 0);
constexpr S inc() { v2 v = {1, max}; ++v[1]; return v[1]; }
static_assert(inc() == 0);
constexpr S compound() { v2 v = {1, 2}; v[0] += max; return v[0]; }
static_assert(compound() == 0);
constexpr S hi() { v3 v = {1, 2, 3}; return v.hi[1]; }
static_assert(hi() == 0);
