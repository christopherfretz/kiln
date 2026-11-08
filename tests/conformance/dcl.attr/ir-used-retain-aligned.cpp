// [dcl.attr] GNU `used` emits an otherwise unreferenced internal entity;
// on ELF it is listed in `@llvm.compiler.used`, and with `retain` also
// in `@llvm.used` (`retain` alone lists an external entity there).
// `aligned(N)` on a function or member function sets its `align` (clang
// `CodeGenModule::addUsedOrCompilerUsedGlobal`,
// `SetLLVMFunctionAttributesForDefinition`; spec/landed/sema/WP-805.md).
// WP-805. clang 19's IR differs in the AArch64 datalayout and
// "frame-pointer" attributes, so this compares with clang 22 and the
// `-19` sibling with clang 19 (WP-839).
//@ requires: clang>=22
//@ expect: ir
__attribute__((used)) static int used_var = 1;
__attribute__((used)) static int used_fn() { return 2; }
__attribute__((used, retain)) static int retained_var = 3;
__attribute__((used, retain)) static int retained_fn() { return 4; }
__attribute__((retain)) int retain_only = 5;
__attribute__((aligned(64))) void aligned_fn() {}
[[gnu::aligned(32)]] void aligned_decl();
void aligned_decl() {}
struct S {
  __attribute__((aligned(128))) void member() {}
};
void use(S *s) { s->member(); }
