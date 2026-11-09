// Debug info (WP-910): `_Complex` base types. A complex floating type is
// `DW_ATE_complex_float`; a complex integer type is clang's
// `DW_ATE_lo_user`, which the IR prints as `encoding: 128` (LLVM's parser
// has no name for it).
//@ args: -g
_Complex float cf;
_Complex double cd;
_Complex int ci;
_Complex long long cll;
long use() { return __real__ ci + __imag__ cll + (long)__real__ cf + (long)__imag__ cd; }
