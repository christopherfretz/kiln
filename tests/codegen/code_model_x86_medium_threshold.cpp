// WP-1419: `-mlarge-data-threshold=0x100` with x86-64
// `-mcmodel=medium`: `Large Data Threshold` 256 (read in radix 0, as
// clang's `-cc1`) instead of the driver's 65536, as clang's
// (spec/driver.md "Codegen flags").
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mcmodel=medium -mlarge-data-threshold=0x100
extern int ext;
static int big[100000];
int g = 1;
int f(int i) { return g + ext + big[i]; }
