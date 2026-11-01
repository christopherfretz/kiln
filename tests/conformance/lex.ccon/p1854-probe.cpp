// [lex.ccon]/3 (C++23, P1854R4): a non-encodable or multi-code-unit
// ordinary character literal is ill-formed. WP-185 feature probe.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:10 error "character too large for enclosing character literal type"
char c = 'é';
