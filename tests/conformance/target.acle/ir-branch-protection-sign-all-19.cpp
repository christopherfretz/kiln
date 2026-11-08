// [driver] Of `-mbranch-protection=` and `-msign-return-address=` the last
// wins, as in clang 19's driver: `-msign-return-address=all` after `bti`
// signs every function with the A key and has no BTI (spec/driver.md
// "Codegen flags"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -mbranch-protection=bti -msign-return-address=all
int g(int);
int f(int x) { return g(x) + 1; }
int leaf(int x) { return x * 2; }
