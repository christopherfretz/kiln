// [driver] [ACLE] `-msve-vector-bits=N` fixes the SVE vector length:
// every function definition carries `vscale_range(N/128,N/128)` instead
// of `vscale_range(1,16)`. spec/driver.md §Target features,
// spec/codegen.md §7u.
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8-a+sve -msve-vector-bits=256

int f(int x) { return x + 1; }
