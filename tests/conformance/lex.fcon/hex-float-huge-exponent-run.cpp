// [lex.fcon]/3 (C++17, P0245R1), [expr.pre]/4: a hexadecimal floating-point
// literal whose binary exponent does not fit in 64 bits is out of range for
// its type and becomes infinity (or zero for a huge negative exponent),
// with clang's -Wliteral-range warning. kiln's exponent saturated but the
// rounding added it unchecked: a debug build crashed (WP-971) and a
// release build wrapped the sum and produced zero. The twin `ok` is in range.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ diag: 15:15 warning "magnitude of floating-point constant too large for type 'float'"
//@ diag: 16:16 warning "magnitude of floating-point constant too large for type 'double'"
//@ diag: 17:17 warning "magnitude of floating-point constant too small for type 'double'"
//@ stdout: inf inf 0 8
extern "C" int printf(const char*, ...);
float big_f = 0xFp100000000000000000000F;
double big_d = 0x1p+99999999999999999999999;
double tiny_d = 0x1p-99999999999999999999999;
double ok = 0x1p3;
int main() { printf("%g %g %g %g\n", big_f, big_d, tiny_d, ok); }
