// [driver] `-mspeculative-load-hardening` on x86 gives every function
// `speculative_load_hardening` (not those marked
// no_speculative_load_hardening) and adds +retpoline-indirect-calls to
// the target features (clang's getX86TargetFeatures), keeping the
// x86-64 baseline; a target("arch=") function keeps the added feature
// (spec/landed/codegen/WP-813.md). WP-813.
//@ requires: clang<22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mspeculative-load-hardening
void g();
void plain() { g(); }
__attribute__((no_speculative_load_hardening)) void off() { g(); }
__attribute__((target("arch=haswell"))) void arch() { g(); }
