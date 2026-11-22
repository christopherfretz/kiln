// [driver] `-mgeneral-regs-only` on AArch64 (clang
// `getAArch64TargetFeatures`, WP-1804): after the `-march=` extensions,
// `fp-armv8` is turned off and with it every feature that needs it (NEON,
// SVE, FP16, the crypto extensions, RDM), giving clang's "target-features"
// function attribute. Integer code is unaffected. The target is pinned;
// clang 22 and 19 agree. spec/driver.md "AArch64 target features".
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8.2-a+sve+crypto -mgeneral-regs-only
int f(int x) { return x + 1; }
long g(long a, long b) { return a * b - f(int(a)); }
