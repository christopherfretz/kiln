// [conv.prom]/1: unsigned short promotes to int, so the product can overflow.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_unsigned_promoted`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:46 note "value 4294836225 is outside the range of representable values of type 'int'"
//@ diag: 9:15 note "in call to 'f(65535)'"
constexpr int f(unsigned short a) { return a * a; }
static_assert(f(65535) == 0);
