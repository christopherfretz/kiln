// [ACLE] `__builtin_arm_get_sme_state` is declared on AArch64 as clang's
// `void(unsigned long *, unsigned long *)` (`BuiltinsAArch64.def`
// "vULi*ULi*", nothrow, no target feature): each operand must convert to
// `unsigned long *`, and the builtin cannot have its address taken
// (spec/codegen.md "Target builtins"). WP-1794.
//@ args: --target=aarch64-unknown-linux-gnu
//@ verify
void f(long *sl, unsigned long *ul, unsigned *u) {
  __builtin_arm_get_sme_state(sl, ul); // expected-error {{cannot initialize a parameter of type 'unsigned long *' with an lvalue of type 'long *'}}
  __builtin_arm_get_sme_state(ul, u); // expected-error {{cannot initialize a parameter of type 'unsigned long *' with an lvalue of type 'unsigned int *'}}
  __builtin_arm_get_sme_state(ul); // expected-error {{too few arguments to function call, expected 2, have 1}}
  __builtin_arm_get_sme_state(nullptr, nullptr);
  __builtin_arm_get_sme_state(ul, ul);
}
auto p = &__builtin_arm_get_sme_state; // expected-error {{builtin functions must be directly called}}
static_assert(noexcept(__builtin_arm_get_sme_state(nullptr, nullptr)));
