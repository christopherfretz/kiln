// C17 6.9p1: a translation unit holds at least one external declaration;
// clang's `ext_empty_translation_unit` (-Wempty-translation-unit, in
// -pedantic) at the end of file (WP-1312, WP-1299 follow-up). C++ has no
// such rule.
//@ args: -pedantic
//@ expect: compile-ok
//@ strict-diags
//@ diag: 9:23 warning "ISO C requires a translation unit to contain at least one declaration"
#define ONLY_A_MACRO 1
