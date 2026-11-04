// [driver] A function's `target("...")` attribute adds its features to
// the command line's in its "target-features": an extension enables its
// LLVM feature and dependencies (`sha3` adds `sha2`, `fp16` is
// `fullfp16`, `fp16fml` adds `fullfp16`), a sub-architecture `v8.Na`
// its level's features, `nosimd` disables `neon` while `no-neon` (the
// LLVM name, not the extension's) changes nothing (spec/landed/codegen/WP-558.md
// "Target features"). Host-restricted (no pinned `--target`; see
// ext.arm.sve/ir-target-features-march-levels.cpp). The clang 22 IR
// (a `-19` sibling pins clang 19's). WP-558.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
__attribute__((target("bf16,neon"))) void f1() {}
__attribute__((target("sha3"))) void f2() {}
__attribute__((target("fp16"))) void f3() {}
__attribute__((target("fp16fml"))) void f4() {}
__attribute__((target("v8.1a"))) void f5() {}
__attribute__((target("v8.3a,fullfp16"))) void f6() {}
__attribute__((target("no-neon"))) void f7() {}
__attribute__((target("dotprod,i8mm"))) void f8() {}
void f9() {}
__attribute__((target("nosimd"))) void f10() {}
