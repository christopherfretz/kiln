// [driver] clang 22's `BuiltinARMSpecialReg` takes the full `op0` (0-3)
// as the `o0` field of a system-register encoding string, so
// `"3:3:14:0:2"` is `cntvct_el0` (clang 19 took at most 1;
// special-register-o0-19.cpp) (spec/codegen.md "Target builtins").
// WP-846.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ verify
void f() {
  (void)__builtin_arm_rsr64("3:3:14:0:2");
  (void)__builtin_arm_rsr64("2:3:14:0:2");
  __builtin_arm_rsr64("4:3:14:0:2"); // expected-error {{invalid special register for builtin}}
}
