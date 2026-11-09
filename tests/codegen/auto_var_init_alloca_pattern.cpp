// -ftrivial-auto-var-init=pattern (spec/codegen.md §7y, "alloca"): clang's
// `initializeAlloca` (CGBuiltin.cpp) fills the memory of
// __builtin_alloca and __builtin_alloca_with_align with one `llvm.memset`
// of the run-time size at the alloca's alignment; the `_uninitialized`
// variants are left alone. The fill byte is clang's pattern, 0xAA;
// [[clang::uninitialized]] on the variable the pointer initializes does
// not apply.
//@ args: -ftrivial-auto-var-init=pattern
void use(void *);
void plain(unsigned long n) { use(__builtin_alloca(n)); }
void aligned(unsigned long n) { use(__builtin_alloca_with_align(n, 64)); }
void uninit(unsigned long n) {
  use(__builtin_alloca_uninitialized(n));
  use(__builtin_alloca_with_align_uninitialized(n, 128));
}
void fixed() {
  [[clang::uninitialized]] void *p = __builtin_alloca(16);
  use(p);
}
void in_loop(int n) {
  for (int i = 0; i < n; ++i)
    use(__builtin_alloca(i + 1));
}
