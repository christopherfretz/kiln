// WP-1419: x86-64 `-mcmodel=kernel` with `-fno-pic`: `Code Model` 2
// and no `Large Data Threshold`, as clang's (spec/driver.md "Codegen
// flags").
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mcmodel=kernel -fno-pic
extern int ext;
static int big[100000];
int g = 1;
int f(int i) { return g + ext + big[i]; }
