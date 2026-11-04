// [driver] __builtin_ia32_pause is an x86-64 target builtin
// (`BuiltinsX86.def` `"v"`, `"n"`: `void () noexcept`), lowered as clang's
// `EmitX86BuiltinExpr` to `call void @llvm.x86.sse2.pause()`, declared
// `nounwind`; the clang 19 module. libstdc++'s <bits/atomic_wait.h> spins
// on it. WP-610.
//@ requires: clang<22
//@ requires: target-x86_64
//@ expect: ir
static_assert(noexcept(__builtin_ia32_pause()));
void spin(volatile int *flag) {
  while (*flag == 0)
    __builtin_ia32_pause();
}
