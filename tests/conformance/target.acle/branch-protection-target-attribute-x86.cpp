// [dcl.attr] On x86 a `target("branch-protection=...")` attribute is
// ignored with clang 19's -Wignored-attributes warning (spec/driver.md
// "Codegen flags"). WP-846.
//@ requires: target-x86_64
//@ verify
__attribute__((target("branch-protection=bti"))) void a() {} // expected-warning {{unsupported 'branch-protection' in the 'target' attribute string; 'target' attribute ignored}}
