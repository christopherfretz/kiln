// [clang ext] `target_version` on a redeclaration after the definition
// (clang `checkNewAttributesAfterDef`). From clang 22 a default version after the definition
// is ignored with a warning; a non-default version and `target_clones` there are accepted
// (the version is hidden). spec/landed/codegen/WP-810.md (WP-810).
//@ expect: compile-ok
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 12:16 warning "attribute declaration must precede definition"
//@ diag: 11:5 note "previous definition is here"
int lt() { return 8; }
__attribute__((target_version("default"))) int lt();
int l2() { return 8; }
__attribute__((target_version("sve"))) int l2();
int l3() { return 8; }
[[gnu::target_clones("sve")]] int l3();
