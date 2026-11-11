// [expr.const]/10, [cpp.predefined]: an overflow in a function template
// of an included header, whose operand is a predefined macro, makes a
// static assertion's condition not a constant expression, with the
// macro's expansion note as without a precompiled header; so does the
// constant-conversion warning of an instantiation whose operand is one
// (its note in `<built-in>` is not checked here) (WP-1310).
//@ expect: compile-fail
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:15 note "in call to 'add<int>(1)'"
#include "Inputs/predef_aligned.h"

static_assert(pa::add(1) > 0);

char c = pa::trunc<char>();
