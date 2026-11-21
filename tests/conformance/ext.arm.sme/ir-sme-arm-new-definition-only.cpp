// [ACLE] `__arm_new("za")` / `__arm_new("zt0")` in IR as clang 19 and 22:
// `"aarch64_new_za"` / `"aarch64_new_zt0"` are attributes of the
// function's definition only, not of its declarations and not of the
// calls to it (direct, after a redeclaration, through a member, and
// combined with `__arm_locally_streaming`, whose `"aarch64_pstate_sm_body"`
// stays on the declaration and the call). spec/types.md §5.4, WP-1826.
// `-mno-outline-atomics`: the features must not depend on the host's
// sysroot; `-ftrivial-auto-var-init=uninitialized`: clang 22 has no C++26
// erroneous values. No header is included under the pinned --target.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2 -mno-outline-atomics -ftrivial-auto-var-init=uninitialized
__arm_new("za") void nz() {}
__arm_new("zt0") void nt();
__arm_new("za", "zt0") void nb() {}
__arm_new("za") __arm_locally_streaming void nls();
__arm_new("za") void later();
void later() {}
struct S { __arm_new("za") void m(); __arm_new("zt0") static void sm() {} };
void S::m() {}
void use(S &s) {
  nz();
  nt();
  nb();
  nls();
  later();
  s.m();
  S::sm();
}
