// [lex.string]/8 (C++23, P2201R1): concatenating string literals with two
// different encoding prefixes is ill-formed. WP-185 feature probe.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 7:26 error "unsupported non-standard concatenation of string literals"
const char8_t* s = u8"a" L"b";
