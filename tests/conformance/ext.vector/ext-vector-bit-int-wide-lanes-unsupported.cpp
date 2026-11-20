// [expr.const]: kiln does not yet evaluate a vector of wide `_BitInt(N)`
// lanes (N > 128), which clang 22 does (ext-vector-bit-int-wide-lanes.cpp).
// Every form fails cleanly with the wide-`_BitInt` note at the vector: a
// lane subscript (read, write, `++`, compound assignment), an element
// access, `__builtin_convertvector` to a narrow vector, a shuffle, a
// splat and an operator (a lane subscript once crashed the Oven); a
// reduction in ext-vector-bit-int-wide-lanes-unsupported-clang22.cpp.
// x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ skip(clang): kiln gap: clang 22 evaluates all but the global's read, clang 19 no lane access
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:15 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 48:14 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 48:18 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:15 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:39 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:15 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:15 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:33 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 55:15 note "in call to 'write()'"
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:31 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 57:15 note "in call to 'inc()'"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:36 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 59:15 note "in call to 'compound()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:32 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 61:15 note "in call to 'elem()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:33 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 63:15 note "in call to 'splat()'"

using S = _BitInt(256);
typedef S v2 __attribute__((ext_vector_type(2)));
typedef int i2 __attribute__((ext_vector_type(2)));

v2 g = {1, 2};
static_assert(g[1] == 2);

constexpr v2 a = {1, 2};
static_assert(a[1] == 2);
static_assert(__builtin_convertvector(a, i2).y == 2);
static_assert(__builtin_shufflevector(a, a, 1, 0)[0] == 2);
static_assert((a + a)[1] == 4);

constexpr bool write() { v2 v = {1, 2}; v[1] = 5; return v[1] == 5; }
static_assert(write());
constexpr bool inc() { v2 v = {1, 2}; ++v[1]; v[0]--; return v[1] == 3; }
static_assert(inc());
constexpr bool compound() { v2 v = {1, 2}; v[1] += 3; return v[1] == 5; }
static_assert(compound());
constexpr bool elem() { v2 v = {1, 2}; v.x = 9; return v.x == 9; }
static_assert(elem());
constexpr bool splat() { v2 v = S(5); return v[1] == 5; }
static_assert(splat());
