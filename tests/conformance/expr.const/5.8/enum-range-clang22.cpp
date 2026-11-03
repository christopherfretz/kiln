// [expr.static.cast]/9, [dcl.enum]/8: converting a value outside the range of
// an unscoped enumeration without a fixed underlying type is undefined. As
// clang 22 does, kiln makes such a value a failed constant evaluation in
// every context that requires a constant (the static_assert included), with
// the range as the note ([expr.const]/5.8). Types with a fixed underlying
// type have its whole range. (enum-range.cpp: clang 19's
// -Wenum-constexpr-conversion behaviour, kept at -fclang-compat=19.)
// Mirrors crates/kiln_oven/tests/ub.rs `ub_enum_range`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:31 note "integer value 5 is outside the valid range of values [0, 1] for the enumeration type 'E'"
//@ diag: 27:15 note "in call to 'f(5)'"
//@ diag: 28:13 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 26:31 note "integer value 5 is outside the valid range of values [0, 1] for the enumeration type 'E'"
//@ diag: 28:17 note "in call to 'f(5)'"
//@ diag: 29:13 error "constexpr variable 'e2' must be initialized by a constant expression"
//@ diag: 26:31 note "integer value 5 is outside the valid range of values [0, 1] for the enumeration type 'E'"
//@ diag: 29:18 note "in call to 'f(5)'"
//@ diag: 30:13 error "constexpr variable 'e3' must be initialized by a constant expression"
//@ diag: 26:31 note "integer value -1 is outside the valid range of values [0, 1] for the enumeration type 'E'"
//@ diag: 30:18 note "in call to 'f(-1)'"
//@ requires: clang>=22
// clang 22 spelling of enum-range.cpp (WP-394).
enum E { A, B };
constexpr E f(int v) { return (E)v; }
static_assert(f(5) == 5);
constexpr E e = f(5);
constexpr E e2 = f(5);
constexpr E e3 = f(-1);
constexpr E ok = f(1);
enum class S { X };
constexpr S s = (S)7;
enum F : unsigned char { P };
constexpr F g = (F)200;
enum G { M = -3, N = 4 };
constexpr G h = (G)-8;
constexpr G h2 = (G)7;
static_assert(h == -8 && h2 == 7);
