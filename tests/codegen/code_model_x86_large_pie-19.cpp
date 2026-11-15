// WP-1419: `-mcmodel=large` on x86-64 with the default PIE is
// accepted (only AArch64 needs `-fno-pic`): `Code Model` 4 and the
// driver's default `Large Data Threshold` 0, as clang's
// (spec/driver.md "Codegen flags").
// The clang 19 golden; the clang 22 one is the sibling without -19.
//@ requires: clang<22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mcmodel=large
extern int ext;
static int big[100000];
int g = 1;
int f(int i) { return g + ext + big[i]; }
