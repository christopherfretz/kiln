// [cpp.predefined]/1, [tab:cpp.predefined.ft] (P2900R14): `__cpp_contracts`
// is 202502L in C++26. kiln predefines it although WP-707 (evaluation) is
// pending: every contract is parsed and analysed, and evaluation is gated
// by -fcontract-evaluation-semantic (docs/STANDARD_SUPPORT.md "Macro gaps").
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 19 nor clang 22 defines __cpp_contracts (no P2900)
//@ oracle: gcc
static_assert(__cpp_contracts == 202502L);
