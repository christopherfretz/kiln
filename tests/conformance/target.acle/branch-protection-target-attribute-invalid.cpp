// [dcl.attr] An invalid `target("branch-protection=...")` spec is an
// error on AArch64, as in clang 19's `checkTargetAttr`; an empty one is
// ignored (spec/driver.md "Codegen flags"). WP-846.
//@ requires: target-aarch64
//@ verify
__attribute__((target("branch-protection=x"))) void a() {} // expected-error {{invalid or misplaced branch protection specification 'x'}}
__attribute__((target("branch-protection=bti+"))) void b() {} // expected-error {{invalid or misplaced branch protection specification '<empty>'}}
__attribute__((target("branch-protection=pac-ret+leaf"))) void c() {}
__attribute__((target("branch-protection="))) void d() {}
