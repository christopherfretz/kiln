// [expr.static.cast]/4, [expr.cast]/4, [expr.const.cast]/3, [conv.lval]/1,
// [expr.comma]/1:
// `static_cast<const T&>(f())` (also the C-style and `const_cast` spellings,
// nested) of a call `f` returning `const T&` is an lvalue of type `const T`;
// initializing a by-value scalar parameter (or returning it, or using it as
// an arithmetic operand) applies the lvalue-to-rvalue conversion: a call, a
// `load`, a call, exactly as for `sink(f())` and for a named lvalue
// operand. A `reinterpret_cast<const U&>` of such an lvalue
// ([expr.reinterpret.cast]/11) is loaded as `U`; so is the call as the
// right operand of a comma ([expr.comma]/1). WP-632: kiln forwarded the
// call's referee address as the cast's value ("call argument 0: expected
// double, found ptr"). The OK shapes before the fix are regression pins.
// Verified against clang 19 and 22. The C++26 erroneous-value fill kiln
// applies by default (spec/codegen.md §7r) is clang's
// `-ftrivial-auto-var-init=zero`, given to both.
//@ requires: clang>=22
//@ requires: llvm>=22
//@ expect: ir
//@ args: -ftrivial-auto-var-init=zero
#include "Inputs/ref-cast-of-ref-call.h"
