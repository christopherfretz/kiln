// clang's `__attribute__((enable_if(cond, msg)))` (AttrDocs "enable_if"): a
// function whose condition, evaluated with the call's arguments, is false is
// not viable, and the candidate note names the message. libc++ 22's
// `_CmpUnspecifiedParam` (<__compare/ordering.h>) uses it to reject a
// comparison-category comparison with anything but literal 0. kiln ignored
// the attribute and accepted `f(1)` until WP-460:
// the M5 cluster "enable_if attribute" (docs/status/libcxx-m5.md,
// reject-other-than-literal-zero.verify).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:18 error "no matching function for call to 'f'"
//@ diag: 13:6 note "candidate disabled: only literal zero"
void f(int n) __attribute__((enable_if(n == 0, "only literal zero"))) {}
void g() { f(0); f(1); }
