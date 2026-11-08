// [driver] `__builtin_arm_clrex()` lowers as clang's
// `EmitAArch64BuiltinExpr`: a call of `llvm.aarch64.clrex` (it takes no
// operand; kiln required one and rejected the call). Found by the
// WP-805 builtin lowering sweep (kiln_driver/tests/builtin_lowering.rs).
//@ requires: target-aarch64
//@ expect: ir
void f() { __builtin_arm_clrex(); }
