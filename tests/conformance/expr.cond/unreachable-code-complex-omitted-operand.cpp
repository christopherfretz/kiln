// [expr.cond], GNU `x ?: y` (the middle operand omitted) on a complex
// operand that is a call: valid, and clang 19/22 give no diagnostic
// under -Wunreachable-code-aggressive. kiln's unreachable-code walk
// folds the condition (clang `EvaluateAsBooleanCondition`) and the Oven
// stops with an internal error on the opaque complex operand. Reduced
// from complex-binary-conditional-ir.cpp in this directory.
//@ args: -Wunreachable-code-aggressive
//@ strict-diags
//@ xfail(kiln): WP-2150 follow-up, folding the condition of a complex `g() ?: y` reaches an Oven internal error (`init_inner` on an OpaqueValueExpr)
_Complex double g();
_Complex double k(_Complex double y) { return g() ?: y; }
