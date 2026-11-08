// [driver] `-mbranch-protection=pac-ret+pc+gcs` emits clang 19's branch protection: the
// `Min` module flags (value 1) and the function attributes on definitions,
// declarations and calls (spec/driver.md "Codegen flags"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -mbranch-protection=pac-ret+pc+gcs
int g(int);
int f(int x) { return g(x) + 1; }
int leaf(int x) { return x * 2; }
