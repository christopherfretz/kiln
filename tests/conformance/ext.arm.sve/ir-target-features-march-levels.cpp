// [driver] `-march=armv9.2-a+crypto+nolse` resolves the architecture
// level's default extensions with LLVM's dependency closure (`+crypto`
// from v8.4 adds sha3 and sm4, `+nolse` removes lse), giving clang's
// "target-features" and, through the level's sve, `vscale_range(1,16)`.
// spec/driver.md §Target features.
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv9.2-a+crypto+nolse

int f(int x) { return x + 1; }
