// [expr.const]: the forms of ext-vector-bit-int-wide-lanes-unsupported.cpp
// that only clang 22 evaluates (clang 19 folds neither a reduction nor a
// lane subscript of a vector prvalue): a reduction of a vector of wide
// `_BitInt(N)` lanes (N > 128) and a lane subscript of its conversion to
// a narrow vector fail cleanly with the wide-`_BitInt` note at the
// vector. x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ skip(clang): kiln gap: clang 22 evaluates these
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:14 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 25:18 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 26:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:36 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:36 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:39 note "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet"

using S = _BitInt(256);
typedef S v2 __attribute__((ext_vector_type(2)));
typedef int i2 __attribute__((ext_vector_type(2)));

constexpr v2 a = {1, 2};
static_assert(__builtin_reduce_add(a) == 3);
static_assert(__builtin_reduce_max(a) == 2);
static_assert(__builtin_convertvector(a, i2)[1] == 2);
