// [ACLE] `__arm_agnostic("sme_za_state")` (clang 20 and later): a
// function type attribute with the one state `sme_za_state`, printed as
// clang 22 does (without a space after the parameter list).
// spec/types.md §5.4, WP-1426.
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2
//@ strict-diags
//@ diag: 12:25 error "unknown state 'za'"
//@ diag: 13:8 error "cannot initialize a variable of type 'void (*)()' with an lvalue of type 'void ()__arm_agnostic(\"sme_za_state\")'"
void f() __arm_agnostic("sme_za_state") {}
void g() __arm_agnostic("za");
void (*p)() = f;
void (*q)() __arm_agnostic("sme_za_state") = f;
