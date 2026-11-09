// [basic.def.odr]/1, [cpp.include]: the precompiled header's declarations
// are part of the translation unit, so redefining one in the test file is
// an error (WP-1023).
//@ expect: compile-fail
//@ pch: Inputs/pch_unguarded.h
//@ diag: 7 error "redefinition of 'pch_value'"
inline constexpr int pch_value = 1;
