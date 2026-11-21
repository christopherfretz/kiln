// [driver] x86-64 target features (spec/driver.md "x86-64 target
// features", WP-1804): `-mgeneral-regs-only` is `-x87,-mmx,-sse` where it
// stands among the feature flags (clang `getX86TargetFeatures`), so the
// `-mavx2` before it is undone and the `-msse2` after it turns SSE and
// SSE2 back on, giving clang's "target-features" function attribute.
// The target is pinned; the features do not depend on the host.
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mavx2 -mgeneral-regs-only -msse2
int f(int x) { return x + 1; }
long g(long a, long b) { return a * b - f(int(a)); }
