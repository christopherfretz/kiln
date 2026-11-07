// P4136R2 (a DR) and P2843R3 ([cpp.line]/3): a #line digit-sequence of 0
// or above 2147483647 is conditionally-supported; kiln, like clang and
// GCC, accepts one that fits in 32 bits.
//@ expect: compile-ok
//@ strict-diags
#line 0
#line 2147483648
static_assert(__LINE__ == 2147483648);
