// Clang ext vectors, [clang 20+] ([expr.const]/10; clang
// VectorExprEvaluator has no case for CK_IntegralToBoolean on vectors):
// converting an integer ext vector to a `bool` ext vector is not a
// constant expression in any form -- a comparison result, a C-style or
// static_cast, an assignment or a return in a constexpr function -- so
// kiln rejects it too (no extension); __builtin_convertvector to a bool
// vector does evaluate (bool-constexpr-convert-run.cpp). WP-466.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ requires: clang>=22
//@ strict-diags
//@ diag: 33 error "constexpr variable 'L2' must be initialized by a constant expression"
//@ diag: 34:23 error "constexpr variable 'L3' must be initialized by a constant expression"
//@ diag: 35:35 error "constexpr variable 'L4' must be initialized by a constant expression"
//@ diag: 36 error "constexpr variable 'L5' must be initialized by a constant expression"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:7 note "subexpression not valid in a constant expression"
//@ diag: 49:15 note "in call to 'asg({0, 1, 2, 0})'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:34 note "subexpression not valid in a constant expression"
//@ diag: 46:35 note "in call to 'conv({0, 1, 2, 0})'"
//@ diag: 50:15 note "in call to 'ret({0, 1, 2, 0})'"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:41 note "subexpression not valid in a constant expression"
//@ diag: 51:15 note "in call to 'cast({0, 1, 2, 0})'"
typedef int i4 __attribute__((ext_vector_type(4)));
typedef unsigned char uc4 __attribute__((ext_vector_type(4)));
typedef bool b4 __attribute__((ext_vector_type(4)));

constexpr i4 I = {0, 1, 2, 0};
constexpr uc4 U = {3, 0, 0, 1};
// A comparison yields an `int` vector (lanes -1 or 0), converted here.
constexpr b4 L2 = I < 2;
constexpr b4 L3 = (b4)I;
constexpr b4 L4 = static_cast<b4>(U);
constexpr b4 L5 = I == I;
constexpr b4 L6 = __builtin_convertvector(I, b4);
static_assert(!L6[0] && L6[1] && L6[2] && !L6[3]);

constexpr bool asg(i4 x) {
  b4 r = {};
  r = x;
  return r[1];
}
constexpr b4 conv(i4 y) { return y; }
constexpr bool ret(i4 x) { return conv(x)[1]; }
constexpr bool cast(i4 x) { return ((b4)x)[2]; }

static_assert(asg(I));
static_assert(ret(I));
static_assert(cast(I));
