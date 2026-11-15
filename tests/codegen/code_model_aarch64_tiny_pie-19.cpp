// WP-1419: `-mcmodel=tiny` on AArch64 with the default PIE: `Code
// Model` 0 beside `PIC Level` / `PIE Level` 2, as clang's
// (spec/driver.md "Codegen flags").
// The clang 19 golden; the clang 22 one is the sibling without -19.
//@ requires: clang<22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -mcmodel=tiny
extern int ext;
static int big[100000];
int g = 1;
int f(int i) { return g + ext + big[i]; }
