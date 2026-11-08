// [dcl.attr] [driver] `target("branch-protection=...")` replaces the
// function's branch protection, the last such item winning, as clang 19
// does: its definition's and declaration's attributes, not the calls'; an
// empty spec keeps the command line's (here `-mbranch-protection=none`;
// spec/driver.md "Codegen flags"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -mbranch-protection=none
__attribute__((target("branch-protection=bti"))) void a() {}
__attribute__((target("branch-protection=pac-ret+leaf+b-key"))) void c() {}
__attribute__((target("branch-protection=none"))) void d() {}
__attribute__((target("branch-protection="))) void e() {}
__attribute__((target("branch-protection=standard"))) void h();
void i() { h(); }
__attribute__((target("branch-protection=pac-ret+pc"))) void j() {}
__attribute__((target("branch-protection=bti, branch-protection=gcs"))) void k() {}
