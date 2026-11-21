// [expr.const]: a comparison, `!`, `&&` and `||` of vectors of wide
// `_BitInt(N)` lanes (N > 128) are evaluated lane by lane, as the narrow
// lanes' are: a comparison or `!` gives all ones for a true lane, `&&` and
// `||` give 1 (spec/oven.md "Vectors"). A read of an indeterminate lane
// fails. clang 22 crashes evaluating a comparison of wide lanes (its result
// lane is narrower than the lanes it computes) and folds `!` of a zero
// wide lane to garbage, so kiln alone checks these; the run-time values
// (the same) are checked by comparing with clang's code generation.
// x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ skip(clang): clang 22 crashes evaluating a comparison of wide _BitInt lanes
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:54 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 45:15 note "in call to 'uninit()'"

using S = _BitInt(256);
using U = unsigned _BitInt(512);
typedef S v2 __attribute__((ext_vector_type(2)));
typedef U u4 __attribute__((ext_vector_type(4)));

constexpr S big = S(1) << 200;
constexpr v2 a = {-big, big};

static_assert((a == v2{-big, 1})[0] == -1 && (a == v2{-big, 1})[1] == 0);
static_assert((a != a)[1] == 0);
static_assert((a < 0)[0] == -1 && (a < 0)[1] == 0);
static_assert((a <= big)[1] == -1 && (a > big)[1] == 0 && (a >= -big)[0] == -1);
static_assert((u4{~U(0), 1, 2, 3} > u4{0, 1, 2, 3})[0] == -1);
static_assert((u4{~U(0), 1, 2, 3} > 1)[1] == 0);
static_assert((!a)[0] == 0 && (!v2{0, 1})[0] == -1);
static_assert((!u4{0, 1, 0, 0})[2] == -1);
static_assert((a && v2{0, 1})[1] == 1 && (a && v2{0, 1})[0] == 0);
static_assert((v2{0, 0} || v2{0, big})[1] == 1 && (v2{0, 0} || v2{0, big})[0] == 0);

constexpr bool order() {
  v2 v = {1, 2}, w = __builtin_shufflevector(v, v, 1, 0);
  return (v < w)[0] == -1 && (v < w)[1] == 0;
}
static_assert(order());

constexpr S uninit() { v2 v; v[0] = 1; return v[0] + v[1]; }
static_assert(uninit() == 1);
