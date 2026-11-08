// [c.math.syn]: `__builtin_nan{,s}f16` and `__builtin_nan{,s}f128` with a
// literal argument fold to the NaN constant, and with any other argument
// call the library function of the same name (clang
// `getBuiltinLibFunction`: `nanf16`, `nansf16`, `nanf128`, `nansf128`);
// kiln rejected the non-literal calls ("not a library builtin"). Found by
// the WP-805 builtin lowering sweep.
//@ expect: ir
const char *s;
auto lit16() { return __builtin_nanf16(""); }
auto lits128() { return __builtin_nansf128("1"); }
auto call16() { return __builtin_nanf16(s); }
auto calls16() { return __builtin_nansf16(s); }
auto call128() { return __builtin_nanf128(s); }
auto calls128() { return __builtin_nansf128(s); }
