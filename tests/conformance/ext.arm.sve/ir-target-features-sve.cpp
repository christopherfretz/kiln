// [driver] [ACLE] `-march=armv8-a+sve` adds `+fullfp16` and `+sve` to the
// AArch64 "target-features" of every function (sorted `+` features, then
// sorted `-`), and with `sve` clang's CodeGenFunction gives each
// definition `vscale_range(1,16)`, including the global-initialization
// functions. spec/driver.md §Target features, spec/codegen.md §7u.
//@ requires: clang>=22
//@ requires: llvm>=22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve

int f(int x) { return x + 1; }
int g = f(2);
