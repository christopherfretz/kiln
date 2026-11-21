// [clang ext] `target_version` on AArch64 without the `fmv` target
// feature hides a non-default version: it is never declared (see
// multiversion-attribute-errors.cpp). A member function or a friend
// cannot be hidden, so kiln rejects such a version there; the default
// version of a member is the member. Clang 19 and 22 accept the
// declarations and crash on the definition of such a member
// (docs/status/deviations.md). spec/sema.md "Function multiversioning"
// (WP-1745).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ skip(clang): clang 19 and 22 accept a hidden member or friend version and crash on a member's definition
//@ strict-diags
//@ diag: 17:18 error "non-default 'target_version' version of a member function is not allowed"
//@ diag: 19:25 error "non-default 'target_version' version of a friend function is not allowed"
struct S {
  __attribute__((target_version("default"))) int m() { return 0; }
  __attribute__((target_version("sve"))) int m() { return 1; }
  __attribute__((target_version("default"))) int n();
  friend __attribute__((target_version("sve"))) int fr();
};
int use(S &s) { return s.m() + s.n(); }
