// [driver] x86-64 target features (spec/driver.md "x86-64 target
// features", WP-1000): `-march=haswell -mno-avx2 -mtune=znver3`: the CPU's
// features less avx2 and everything that depends on it, giving clang's
// "target-cpu" / "target-features" (and "tune-cpu") function attributes.
// The target is pinned; the features do not depend on the host.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -march=haswell -mno-avx2 -mtune=znver3

int f(int x) { return x + 1; }
__attribute__((target("avx512f"))) int g(int x) { return x * 2; }
