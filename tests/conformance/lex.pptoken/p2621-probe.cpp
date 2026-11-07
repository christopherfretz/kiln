// P2621R3 ([lex.phases], a DR): an incomplete universal-character-name
// is not undefined behavior; clang lexes `\` and the rest separately and
// warns.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
//@ diag: 8:11 warning "incomplete universal character name; treating as '\\' followed by identifier"
#define A \u00
int x;
