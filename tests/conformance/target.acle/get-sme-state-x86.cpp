// [ACLE] `__builtin_arm_get_sme_state` is an AArch64 target builtin: on
// x86-64 the name is undeclared and `__has_builtin` is 0, as clang's
// (spec/codegen.md "Target builtins"). WP-1794.
//@ args: --target=x86_64-unknown-linux-gnu
//@ verify
static_assert(!__has_builtin(__builtin_arm_get_sme_state));
void f(unsigned long *a) {
  __builtin_arm_get_sme_state(a, a); // expected-error {{use of undeclared identifier '__builtin_arm_get_sme_state'}}
}
