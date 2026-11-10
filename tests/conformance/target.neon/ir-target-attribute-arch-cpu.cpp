// [driver] A function's `target("arch=...")` makes the level its base
// architecture with the level's default extensions, then its `+ext`
// modifiers; `cpu=` is the function's "target-cpu" (dropping the command
// line's "tune-cpu") with the CPU's extensions; `tune=` is its
// "tune-cpu"; a raw LLVM feature (`spe`, `perfmon`, an unknown name)
// passes through as `+x` (`no-x` as `-x`). clang
// `AArch64TargetInfo::parseTargetAttr` (spec/driver.md "Target
// features", target attribute). Host-restricted (no pinned `--target`).
// The clang 22 IR, where a function with `sve` gets `vscale_range`
// (a `-19` sibling pins clang 19's). WP-1187.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
__attribute__((target("arch=armv8.2-a+sve"))) void a1() {}
__attribute__((target("cpu=neoverse-v1"))) void a2() {}
__attribute__((target("tune=neoverse-v1"))) void a3() {}
__attribute__((target("arch=armv8.2-a+dotprod+nofp16"))) void a4() {}
__attribute__((target("arch=armv9-a+nosve2"))) void a5() {}
__attribute__((target("cpu=neoverse-v1+nosve"))) void a6() {}
__attribute__((target("arch=armv8.4-a,cpu=cortex-a57"))) void a7() {}
__attribute__((target("cpu=cortex-a57,tune=neoverse-v1"))) void a8() {}
__attribute__((target("spe"))) void a9() {}
__attribute__((target("perfmon"))) void a10() {}
__attribute__((target("+sve2+nofp16"))) void a11() {}
__attribute__((target("arch=armv8.1-a,sve"))) void a12() {}
__attribute__((target("arch=armv8-a+crypto"))) void a14() {}
__attribute__((target("arch=armv8.4-a+crypto"))) void a15() {}
__attribute__((target("arch=armv8.2-a+bogus"))) void a16() {}
__attribute__((target("bogusfeat"))) void a17() {}
__attribute__((target("+bogus"))) void a18() {}
__attribute__((target("no-spe"))) void a19() {}
__attribute__((target("arch=armv8.2a"))) void a20() {}
__attribute__((target("cpu=cortex-r82"))) void a21() {}
__attribute__((target(" arch=armv8.3-a , dotprod "))) void a22() {}
void plain() {}
