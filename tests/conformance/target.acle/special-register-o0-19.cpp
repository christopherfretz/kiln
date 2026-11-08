// [driver] Under `-fclang-compat=19` the `o0` field of a system-register
// encoding string is at most 1, as clang 19's `BuiltinARMSpecialReg`
// (clang 22 takes the full `op0`, 0-3; special-register-o0.cpp)
// (spec/codegen.md "Target builtins"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ verify
void f() {
  (void)__builtin_arm_rsr64("1:3:14:0:2");
  __builtin_arm_rsr64("2:3:14:0:2"); // expected-error {{invalid special register for builtin}}
  __builtin_arm_rsr64("3:3:14:0:2"); // expected-error {{invalid special register for builtin}}
}
