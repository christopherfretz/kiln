// [driver] A function's `target("...")` over the command line's
// `-march=armv9-a+sve -mtune=cortex-a76`: the module's extensions seed
// the set, so `arch=` adds to them and `+nosve` removes one, a
// sub-level `arch=` keeps the module's higher-level features, `cpu=`
// replaces the "tune-cpu", and `nosve2` disables the module's `sve2`.
// clang `AArch64TargetInfo::parseTargetAttr` with
// `reconstructFromParsedFeatures` (spec/driver.md "Target features",
// target attribute). Host-restricted (no pinned `--target`). The clang
// 19 IR (sibling of the clang 22 test). WP-1187.
//@ requires: clang<22
//@ requires: target-aarch64
//@ args: -march=armv9-a+sve -mtune=cortex-a76
//@ expect: ir
__attribute__((target("arch=armv8.2-a+nosve"))) void c1() {}
__attribute__((target("cpu=cortex-a57"))) void c2() {}
__attribute__((target("tune=cortex-a57"))) void c3() {}
__attribute__((target("arch=armv8.1-a+dotprod"))) void c4() {}
__attribute__((target("nosve2"))) void c5() {}
void c6() {}
