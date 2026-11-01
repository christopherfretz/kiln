// [lex.name]/1 (C++23 DR, P1949R7): identifiers follow UAX #31; an emoji
// is neither XID_Start nor XID_Continue. WP-185 feature probe.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:6 error "character <U+1F600> not allowed in an identifier"
int café = 1;
int x٣ = 2;
int b😀 = 3;
