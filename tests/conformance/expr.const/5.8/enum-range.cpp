// [expr.static.cast]/9, [dcl.enum]/8: converting a value outside the range of
// an unscoped enumeration without a fixed underlying type is undefined. As
// clang 19 does, kiln accepts the value and reports the conversion as an
// error (-Wenum-constexpr-conversion) where a constant expression proper is
// required (a constexpr initializer), on every such evaluation, but not in
// an integral constant expression (the static_assert). Types with a fixed
// underlying type have its whole range.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_enum_range`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:31 error "integer value 5 is outside the valid range of values [0, 1] for the enumeration type 'E'"
//@ diag: 15:31 error "integer value 5 is outside the valid range of values [0, 1] for the enumeration type 'E'"
//@ diag: 15:31 error "integer value -1 is outside the valid range of values [0, 1] for the enumeration type 'E'"
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
