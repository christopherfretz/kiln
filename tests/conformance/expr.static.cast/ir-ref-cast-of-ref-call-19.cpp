// [expr.static.cast]/4, [expr.cast]/4, [conv.lval]/1: the clang 19 IR
// (sibling of ir-ref-cast-of-ref-call.cpp, which says what is pinned).
// WP-632.
//@ requires: clang<22
//@ expect: ir
//@ args: -ftrivial-auto-var-init=zero
#include "Inputs/ref-cast-of-ref-call.h"
