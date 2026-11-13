// [cpp.pragma]: '#pragma clang attribute push (attr, apply_to = subject)'
// applies the attribute to every matching declaration until the pop
// (clang ActOnPragmaAttributeAttribute). Found while testing PCH prefixes
// that end inside a push (WP-1394).
//@ expect: compile-ok
//@ xfail(kiln): WP-1394 follow-up, '#pragma clang attribute' is rejected as not supported yet
#pragma clang attribute push (__attribute__((annotate("pfx"))), apply_to = function)
int annotated() { return 1; }
#pragma clang attribute pop
int plain() { return annotated(); }
