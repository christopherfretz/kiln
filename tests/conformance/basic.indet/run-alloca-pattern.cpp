//@ expect: run
//@ args: -ftrivial-auto-var-init=pattern
//@ stdout: alloca 0
//@ stdout: aligned 0
//@ stdout: uninitialized-variant compiles
// -ftrivial-auto-var-init=pattern (spec/driver.md, spec/codegen.md §7y
// "alloca"): as clang's `initializeAlloca`, every byte of the memory of
// __builtin_alloca and __builtin_alloca_with_align is the 0xAA pattern
// (printed: the count of bytes that are not). An earlier frame dirties
// the same stack first, so a missing fill shows. The `_uninitialized`
// variants are not filled; their contents are not printed. Both
// compilers run it (the flag is clang's).
extern "C" int printf(const char *, ...);
[[gnu::noinline]] void dirty() {
  volatile unsigned char junk[1024];
  for (int k = 0; k < 1024; ++k)
    junk[k] = 0x5A;
}
static int not_pattern(const unsigned char *b, unsigned long n) {
  int s = 0;
  for (unsigned long k = 0; k < n; ++k)
    s += b[k] != 0xAA;
  return s;
}
[[gnu::noinline]] int read_alloca(unsigned long n) {
  return not_pattern(static_cast<unsigned char *>(__builtin_alloca(n)), n);
}
[[gnu::noinline]] int read_aligned(unsigned long n) {
  auto *p = __builtin_alloca_with_align(n, 128);
  return not_pattern(static_cast<unsigned char *>(p), n);
}
[[gnu::noinline]] void uninit(unsigned long n) {
  auto *p = static_cast<unsigned char *>(__builtin_alloca_uninitialized(n));
  p[0] = 1;
  auto *q = static_cast<unsigned char *>(__builtin_alloca_with_align_uninitialized(n, 64));
  q[0] = p[0];
  printf("uninitialized-variant compiles\n");
}
int main() {
  dirty();
  printf("alloca %d\n", read_alloca(200));
  dirty();
  printf("aligned %d\n", read_aligned(300));
  uninit(16);
}
