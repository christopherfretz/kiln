// [clang ext] `target_version` on a redeclaration after the definition
// (clang `checkNewAttributesAfterDef`). Clang 19 accepts a default version
// after the definition silently, a non-default version and `target_clones`
// too (the version is hidden). spec/landed/codegen/WP-810.md (WP-810).
//@ expect: compile-ok
//@ requires: clang<22
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
int lt() { return 8; }
__attribute__((target_version("default"))) int lt();
int l2() { return 8; }
__attribute__((target_version("sve"))) int l2();
int l3() { return 8; }
[[gnu::target_clones("sve")]] int l3();
