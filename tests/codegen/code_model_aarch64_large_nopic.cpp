// WP-1419: `-mcmodel=large` on AArch64 with `-fno-pic` (the only
// relocation model clang accepts it with): `Code Model` 4 and no
// `Large Data Threshold` (an x86-64 flag), as clang's
// (spec/driver.md "Codegen flags").
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -mcmodel=large -fno-pic
extern int ext;
static int big[100000];
int g = 1;
int f(int i) { return g + ext + big[i]; }
