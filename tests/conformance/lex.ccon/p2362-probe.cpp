// [lex.ccon]/3 (C++23, P2362R3): a multicharacter wide character literal,
// and a wide character literal whose character is not encodable in one code
// unit, are ill-formed. WP-185 feature probe.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 8:13 error "wide character literals may not contain multiple characters"
wchar_t w = L'ab';
