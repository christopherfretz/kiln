// [ACLE] `__builtin_arm_get_sme_state(p0, p1)` lowers as clang's
// `EmitAArch64BuiltinExpr`: a plain call (not an `invoke` inside a try
// block, and no `nounwind`) of the SME ABI support routine
// `{ i64, i64 } @__arm_sme_state()` with the calling convention
// `aarch64_sme_preservemost_from_x2` and the call-site attribute
// `"aarch64_pstate_sm_compatible"`; field 0 is stored through `p0` and
// field 1 through `p1`, each pointer evaluated after the call (spec/codegen.md
// "Target builtins"). The builtin needs no target feature. WP-1794.
// No header is included under the pinned --target;
// `-ftrivial-auto-var-init=uninitialized`: clang has no C++26 erroneous
// values.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -mno-outline-atomics -ftrivial-auto-var-init=uninitialized
void raw(unsigned long *a, unsigned long *b) { __builtin_arm_get_sme_state(a, b); }

struct S { unsigned long x0, x1; };
void member(S &s) { __builtin_arm_get_sme_state(&s.x0, &s.x1); }

static inline __attribute__((always_inline)) bool has_sme() __arm_streaming_compatible {
  unsigned long x0, x1;
  __builtin_arm_get_sme_state(&x0, &x1);
  return x0 & (1UL << 63);
}
bool query() __arm_streaming_compatible { return has_sme(); }

void g();
unsigned long in_try() {
  unsigned long x0 = 0, x1 = 0;
  try {
    g();
    __builtin_arm_get_sme_state(&x0, &x1);
  } catch (...) {
  }
  return x1;
}

constexpr int has = __has_builtin(__builtin_arm_get_sme_state);
static_assert(has == 1);
